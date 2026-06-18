//! OpenAI 兼容协议 Provider 实现。
//!
//! 实现 [`AiProvider`] trait，处理 `/v1/chat/completions` 接口的
//! HTTP POST 请求构造、Bearer 认证、SSE 流式响应解析和错误转换。

use crate::models::ai::{AiProvider, ChatRequest, SendMessageParams, StreamEvent};
use crate::models::error::{AppError, AuthFailReason};
use crate::services::ai::streaming::{extract_error_from_body, parse_sse_stream};
use crate::services::paths;
use super::retry::should_retry_result;
pub use super::retry::{
    compute_backoff_delay, should_retry_err_response, should_retry_error,
    should_retry_ok_response, should_retry_status,
};
use reqwest::Client;
use reqwest::header::HeaderValue;
use tokio::sync::mpsc;

/// OpenAI 兼容协议 Provider。
///
/// 支持所有兼容 OpenAI `/v1/chat/completions` + SSE 流式接口的服务端，
/// 包括 DeepSeek、OpenAI 及各类中转服务。
pub struct OpenAiCompatibleProvider;

/// 构造 ChatRequest 请求体。
#[inline]
pub fn build_chat_request(params: &SendMessageParams) -> ChatRequest {
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

    ChatRequest {
        model: params.model.clone(),
        messages: params.messages.clone(),
        stream: true,
        temperature: Some(temperature),
        top_p: Some(top_p),
        max_tokens,
        frequency_penalty,
        presence_penalty,
        stop,
    }
}

/// 解析 API Key 并构造认证请求头。
///
/// 成功返回 `Some(HeaderMap)`，失败返回 `None`（由调用方发送错误事件）。
#[inline]
pub fn resolve_auth_headers(
    params: &SendMessageParams,
) -> Option<reqwest::header::HeaderMap> {
    let api_key = params.provider.resolve_api_key(&params.provider_key)?;
    let auth_header = HeaderValue::from_str(&format!("Bearer {}", api_key)).ok()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Authorization", auth_header);
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    Some(headers)
}

/// 带指数退避的 HTTP 请求重试。
///
/// 对 429/5xx 状态码或连接超时/网络错误自动重试最多 `max_retries` 次。
#[inline]
async fn send_with_retry(
    client: &Client,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    request: &ChatRequest,
    max_retries: u32,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        let result = client
            .post(url)
            // 不设 .timeout()：它覆盖整个请求+响应体读取，
            // 对流式响应不合适（模型生成可能需要数分钟）。
            // 连接超时已在 Client::builder().connect_timeout() 中设置。
            .headers(headers.clone())
            .json(request)
            .send()
            .await;

        if !should_retry_result(&result, attempt, max_retries) {
            break result;
        }
        tokio::time::sleep(compute_backoff_delay(attempt)).await;
    }
}

/// 处理非成功 HTTP 状态码，转换为对应的 `AppError`。
#[inline]
pub async fn handle_error_response(
    response: reqwest::Response,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) {
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
}

impl AiProvider for OpenAiCompatibleProvider {
    /// 发送流式聊天补全请求。
    ///
    /// 构造标准 OpenAI 兼容格式的 HTTP POST 请求，
    /// 成功时委托 `parse_sse_stream()` 处理 SSE 响应流。
    async fn send_stream(
        &self,
        client: &Client,
        params: SendMessageParams,
        tx: mpsc::UnboundedSender<StreamEvent>,
    ) {
        let url = format!("{}/v1/chat/completions", params.provider.base_url.trim_end_matches('/'));
        let request = build_chat_request(&params);

        if let Err(app_err) = resolve_and_send(client, &url, &params, &request, &tx).await {
            let _ = tx.send(StreamEvent::Error(app_err));
        }
    }
}

/// 解析认证头、发送请求并处理响应。
///
/// 成功时解析 SSE 流，失败时返回对应的 `AppError`。
async fn resolve_and_send(
    client: &Client,
    url: &str,
    params: &SendMessageParams,
    request: &ChatRequest,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> Result<(), AppError> {
    let headers = resolve_auth_headers(params).ok_or(AppError::Auth {
        reason: AuthFailReason::InvalidKeyFormat,
    })?;

    let response = send_with_retry(client, url, &headers, request, 3).await
        .map_err(|e| AppError::Network { detail: e.to_string() })?;

    if !response.status().is_success() {
        handle_error_response_no_consume(response, tx).await;
        return Ok(());
    }

    parse_sse_stream(response, tx.clone()).await;
    Ok(())
}

/// 处理非成功 HTTP 状态码，转换为对应的 `AppError` 并发送到 channel。
async fn handle_error_response_no_consume(
    response: reqwest::Response,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) {
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
}
