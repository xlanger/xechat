//! OpenAI 兼容协议 Provider 实现。
//!
//! 实现 [`AiProvider`] trait，处理 `/v1/chat/completions` 接口的
//! HTTP POST 请求构造、Bearer 认证、SSE 流式响应解析和错误转换。

use crate::models::ai::{AiProvider, ChatRequest, SendMessageParams, StreamEvent};
use crate::models::error::{AppError, AuthFailReason};
use crate::services::ai::streaming::{extract_error_from_body, parse_sse_stream};
use crate::services::paths;
use reqwest::Client;
use reqwest::header::HeaderValue;
use tokio::sync::mpsc;

/// OpenAI 兼容协议 Provider。
///
/// 支持所有兼容 OpenAI `/v1/chat/completions` + SSE 流式接口的服务端，
/// 包括 DeepSeek、OpenAI 及各类中转服务。
pub struct OpenAiCompatibleProvider;

impl AiProvider for OpenAiCompatibleProvider {
    /// 发送流式聊天补全请求。
    ///
    /// 构造标准 OpenAI 兼容格式的 HTTP POST 请求，
    /// 成功时委托 `parse_sse_stream()` 处理 SSE 响应流。
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP 客户端实例
    /// * `params` - 发送参数（base_url/api_key/model/messages/temperature/top_p）
    /// * `tx` - 用于推送 StreamEvent 的 channel 发送端
    ///
    /// # Errors
    ///
    /// 通过 `StreamEvent::Error` 推送 [`AppError`]：
    /// - [`AppError::Auth`] — API Key 格式无效或 401 认证失败
    /// - [`AppError::Network`] — 网络请求失败（连接超时、DNS 解析失败等）
    /// - [`AppError::Api`] — 非 2xx 状态码（尝试从响应体提取可读错误信息）
    async fn send_stream(
        &self,
        client: &Client,
        params: SendMessageParams,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) {
        let url = format!("{}/v1/chat/completions", params.provider.base_url.trim_end_matches('/'));

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
                // 不设 .timeout()：它覆盖整个请求+响应体读取，
                // 对流式响应不合适（模型生成可能需要数分钟）。
                // 连接超时已在 Client::builder().connect_timeout() 中设置。
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
