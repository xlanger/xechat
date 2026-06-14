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

/// 判断 HTTP 状态码是否应触发重试（429 或 5xx）。
#[inline]
pub fn should_retry_status(status: reqwest::StatusCode) -> bool {
    let code = status.as_u16();
    code == 429 || code >= 500
}

/// 判断 reqwest 错误是否应触发重试（超时或连接错误）。
#[inline]
pub fn should_retry_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

/// 判断成功的 HTTP 响应是否应触发重试（可重试状态码且未超过最大重试次数）。
#[inline]
pub fn should_retry_ok_response(status: reqwest::StatusCode, attempt: u32, max_retries: u32) -> bool {
    should_retry_status(status) && attempt < max_retries
}

/// 判断请求错误是否应触发重试（可重试错误且未超过最大重试次数）。
#[inline]
pub fn should_retry_err_response(error: &reqwest::Error, attempt: u32, max_retries: u32) -> bool {
    should_retry_error(error) && attempt < max_retries
}

/// 计算指数退避延迟：500ms * 2^(attempt-1)。
#[inline]
pub fn compute_backoff_delay(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1))
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
        match client
            .post(url)
            .headers(headers.clone())
            .json(request)
            .send()
            .await
        {
            Ok(resp) => {
                if should_retry_ok_response(resp.status(), attempt, max_retries) {
                    tokio::time::sleep(compute_backoff_delay(attempt)).await;
                    continue;
                }
                break Ok(resp);
            }
            Err(e) => {
                if should_retry_err_response(&e, attempt, max_retries) {
                    tokio::time::sleep(compute_backoff_delay(attempt)).await;
                    continue;
                }
                break Err(e);
            }
        }
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
