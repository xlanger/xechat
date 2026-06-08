//! OpenAI Responses API Provider 实现。
//!
//! 实现 [`AiProvider`] trait，处理 OpenAI `/v1/responses` 接口的
//! HTTP POST 请求构造、Bearer 认证、SSE 具名事件流式响应解析。
//!
//! Responses API 与 Chat Completions 的主要差异：
//! - 请求体使用 `input` 替代 `messages`
//! - 流式响应使用具名 SSE 事件（`event:` 行）
//! - 文本增量事件类型为 `response.output_text.delta`

use crate::models::ai::{AiProvider, SendMessageParams, StreamEvent};
use crate::models::error::{AppError, AuthFailReason};
use crate::services::ai::streaming::extract_error_from_body;
use crate::services::paths;
use futures_util::StreamExt;
use reqwest::Client;
use reqwest::header::HeaderValue;
use tokio::sync::mpsc;

/// OpenAI Responses API Provider。
pub struct OpenAiProvider;

impl AiProvider for OpenAiProvider {
    async fn send_stream(
        &self,
        client: &Client,
        params: SendMessageParams,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) {
        let url = format!(
            "{}/v1/responses",
            params.provider.base_url.trim_end_matches('/')
        );

        // Responses API 使用 input 替代 messages，但格式兼容
        let input: Vec<serde_json::Value> = params
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": params.model,
            "input": input,
            "stream": true,
        });

        if let Some(t) = params.temperature {
            body["temperature"] = serde_json::json!(t);
        }
        if let Some(p) = params.top_p {
            body["top_p"] = serde_json::json!(p);
        }
        if let Some(mc) = &params.model_config {
            body["max_tokens"] = serde_json::json!(mc.max_tokens);
            body["frequency_penalty"] = serde_json::json!(mc.frequency_penalty);
            body["presence_penalty"] = serde_json::json!(mc.presence_penalty);
            if !mc.stop_sequences.is_empty() {
                body["stop"] = serde_json::json!(mc.stop_sequences);
            }
        }

        let api_key = match params.provider.resolve_api_key(&params.provider_key) {
            Some(key) => key,
            None => {
                let _ = tx.send(StreamEvent::Error(AppError::Auth {
                    reason: AuthFailReason::InvalidKeyFormat,
                }));
                return;
            }
        };

        let mut headers = reqwest::header::HeaderMap::new();
        let auth_header = match HeaderValue::from_str(&format!("Bearer {}", api_key)) {
            Ok(h) => h,
            Err(_) => {
                let _ = tx.send(StreamEvent::Error(AppError::Auth {
                    reason: AuthFailReason::InvalidKeyFormat,
                }));
                return;
            }
        };
        headers.insert("Authorization", auth_header);
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));

        let max_retries: u32 = 3;
        let mut attempt: u32 = 0;

        let response = loop {
            attempt += 1;
            match client
                .post(&url)
                .headers(headers.clone())
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if (status.as_u16() == 429 || status.as_u16() >= 500) && attempt < max_retries {
                        let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    break Ok(resp);
                }
                Err(e) => {
                    if (e.is_timeout() || e.is_connect()) && attempt < max_retries {
                        let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    break Err(e);
                }
            }
        };

        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(AppError::Network { detail: e.to_string() }));
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();

            let app_err = if status.as_u16() == 401 {
                AppError::Auth {
                    reason: AuthFailReason::Unauthorized {
                        config_path: paths::get_config_path()
                            .to_string_lossy()
                            .to_string(),
                    },
                }
            } else {
                let fallback = extract_error_from_body(&body_text);
                AppError::Api {
                    status: status.as_u16(),
                    body: fallback.or({
                        if body_text.is_empty() { None } else { Some(body_text) }
                    }),
                }
            };

            let _ = tx.send(StreamEvent::Error(app_err));
            return;
        }

        parse_responses_stream(response, tx).await;
    }
}

/// 解析 OpenAI Responses API 的 SSE 具名事件流。
///
/// Responses API 的 SSE 格式与 Chat Completions 不同：
/// - 每个事件有 `event:` 行指定事件类型
/// - `data:` 行携带 JSON 数据
///
/// 关键事件类型：
/// - `response.output_text.delta` → 文本增量 → StreamEvent::Chunk
/// - `response.reasoning.delta` → 推理增量 → StreamEvent::ReasoningChunk
/// - `response.completed` → 完成 → StreamEvent::Complete
/// - `response.error` → 错误 → StreamEvent::Error
async fn parse_responses_stream(
    response: reqwest::Response,
    tx: mpsc::UnboundedSender<StreamEvent>,
) {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut current_event = String::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                let lines: Vec<String> = buffer.lines().map(|l| l.to_owned()).collect();
                buffer.clear();

                for (i, line) in lines.iter().enumerate() {
                    let trimmed = line.trim();

                    if let Some(event_type) = trimmed.strip_prefix("event:") {
                        current_event = event_type.trim().to_string();
                    } else if let Some(data) = trimmed.strip_prefix("data:") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            let _ = tx.send(StreamEvent::Complete);
                            return;
                        }

                        match current_event.as_str() {
                            "response.output_text.delta" => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(delta) = json["delta"].as_str() {
                                        let _ = tx.send(StreamEvent::Chunk(delta.to_string()));
                                    }
                                }
                            }
                            "response.reasoning.delta" => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    if let Some(delta) = json["delta"].as_str() {
                                        let _ = tx.send(StreamEvent::ReasoningChunk(delta.to_string()));
                                    }
                                }
                            }
                            "response.completed" => {
                                let _ = tx.send(StreamEvent::Complete);
                                return;
                            }
                            "response.error" => {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                                    let msg = json["error"]["message"]
                                        .as_str()
                                        .unwrap_or("Unknown error")
                                        .to_string();
                                    let _ = tx.send(StreamEvent::Error(AppError::Api {
                                        status: 0,
                                        body: Some(msg),
                                    }));
                                }
                                return;
                            }
                            // 忽略其他事件类型（response.created, response.output_item.added, response.output_text.done 等）
                            _ => {}
                        }
                        current_event.clear();
                    } else if trimmed.is_empty() {
                        // 空行分隔事件，重置 event 类型
                        current_event.clear();
                    } else if i == lines.len() - 1 {
                        // 不完整行，保留到下次
                        buffer.push_str(trimmed);
                        buffer.push('\n');
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(AppError::Stream { detail: e.to_string() }));
                return;
            }
        }
    }

    let _ = tx.send(StreamEvent::Complete);
}
