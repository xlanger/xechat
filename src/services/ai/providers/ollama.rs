//! Ollama 聊天 Provider 实现。
//!
//! 实现 [`AiProvider`] trait，处理 Ollama `/api/chat` 接口的
//! HTTP POST 请求构造、NDJSON 流式响应解析和错误转换。

use crate::models::ai::{AiProvider, SendMessageParams, StreamEvent};
use crate::models::error::AppError;
use crate::services::ai::streaming::extract_error_from_body;
use reqwest::Client;
use tokio::sync::mpsc;

pub struct OllamaProvider;

impl AiProvider for OllamaProvider {
    async fn send_stream(
        &self,
        client: &Client,
        params: SendMessageParams,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) {
        let url = format!(
            "{}/api/chat",
            params.provider.base_url.trim_end_matches('/')
        );

        let messages: Vec<serde_json::Value> = params
            .messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        let body = serde_json::json!({
            "model": params.model,
            "messages": messages,
            "stream": true,
            "options": {
                "temperature": params.temperature.unwrap_or(0.7),
            }
        });

        let max_retries: u32 = 3;
        let mut attempt: u32 = 0;

        let response = loop {
            attempt += 1;
            match client
                .post(&url)
                // 不设 .timeout()：它覆盖整个请求+响应体读取，
                // 对流式响应不合适（模型生成可能需要数分钟）。
                // 连接超时已在 Client::builder().connect_timeout() 中设置。
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
                let _ = tx.send(StreamEvent::Error(AppError::Network {
                    detail: e.to_string(),
                }));
                return;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            let fallback = extract_error_from_body(&body_text);
            let _ = tx.send(StreamEvent::Error(AppError::Api {
                status: status.as_u16(),
                body: fallback.or(if body_text.is_empty() {
                    None
                } else {
                    Some(body_text)
                }),
            }));
            return;
        }

        parse_ollama_stream(response, tx).await;
    }
}

async fn parse_ollama_stream(
    response: reqwest::Response,
    tx: mpsc::UnboundedSender<StreamEvent>,
) {
    use futures_util::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                let lines: Vec<&str> = buffer.lines().collect();
                let mut remaining = String::new();

                for (i, line) in lines.iter().enumerate() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if i == lines.len() - 1 && !line.ends_with('}') {
                        remaining.push_str(line);
                        remaining.push('\n');
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                        if json["done"].as_bool() == Some(true) {
                            let _ = tx.send(StreamEvent::Complete);
                            return;
                        }
                        if let Some(content) = json["message"]["content"].as_str() {
                            if !content.is_empty() {
                                let _ = tx.send(StreamEvent::Chunk(content.to_string()));
                            }
                        }
                        // Ollama 不同模型使用不同字段名表示推理过程：
                        // - qwen3/deepseek-r1 等使用 "thinking"
                        // - 部分模型使用 "reasoning_content"
                        let reasoning_text = json["message"]["thinking"].as_str()
                            .or_else(|| json["message"]["reasoning_content"].as_str());
                        if let Some(reasoning) = reasoning_text {
                            if !reasoning.is_empty() {
                                let _ = tx.send(StreamEvent::ReasoningChunk(reasoning.to_string()));
                            }
                        }
                    }
                }
                buffer = remaining;
            }
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(AppError::Stream {
                    detail: e.to_string(),
                }));
                return;
            }
        }
    }

    let _ = tx.send(StreamEvent::Complete);
}
