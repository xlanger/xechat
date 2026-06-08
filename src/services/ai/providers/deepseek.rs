//! DeepSeek Chat Completions Provider 实现。
//!
//! 实现 [`AiProvider`] trait，处理 DeepSeek `/v1/chat/completions` 接口的
//! HTTP POST 请求构造、Bearer 认证、SSE 流式响应解析。
//! DeepSeek 在 SSE delta 中返回 `reasoning_content` 字段，
//! 由 `parse_sse_stream()` 统一处理。

use crate::models::ai::{AiProvider, ChatRequest, SendMessageParams, StreamEvent};
use crate::models::error::{AppError, AuthFailReason};
use crate::services::ai::streaming::{extract_error_from_body, parse_sse_stream};
use crate::services::paths;
use reqwest::Client;
use reqwest::header::HeaderValue;
use tokio::sync::mpsc;

/// DeepSeek Chat Completions Provider。
pub struct DeepSeekProvider;

impl AiProvider for DeepSeekProvider {
    async fn send_stream(
        &self,
        client: &Client,
        params: SendMessageParams,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) {
        let url = format!(
            "{}/v1/chat/completions",
            params.provider.base_url.trim_end_matches('/')
        );

        let temperature = params.temperature.unwrap_or(0.7);
        let top_p = params.top_p.unwrap_or(0.9);
        let (frequency_penalty, presence_penalty, stop, max_tokens) = params.model_config.as_ref()
            .map(|mc| {
                (
                    Some(mc.frequency_penalty),
                    Some(mc.presence_penalty),
                    mc.stop_sequences.clone(),
                    Some(mc.max_tokens),
                )
            })
            .unwrap_or((None, None, Vec::new(), None));

        let request = ChatRequest {
            model: params.model.clone(),
            messages: params.messages,
            stream: true,
            temperature: Some(temperature),
            top_p: Some(top_p),
            max_tokens,
            frequency_penalty,
            presence_penalty,
            stop,
        };

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
                .json(&request)
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

        parse_sse_stream(response, tx).await;
    }
}
