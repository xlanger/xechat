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
use super::retry::should_retry_result;
pub use super::retry::{
    compute_backoff_delay, should_retry_err_response, should_retry_error,
    should_retry_ok_response, should_retry_status,
};
use futures_util::StreamExt;
use reqwest::Client;
use reqwest::header::HeaderValue;
use tokio::sync::mpsc;

/// OpenAI Responses API Provider。
pub struct OpenAiProvider;

/// 构造 OpenAI Responses API 请求体。
#[inline]
pub fn build_request_body(params: &SendMessageParams) -> serde_json::Value {
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

    body
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
    body: &serde_json::Value,
    max_retries: u32,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        let result = client
            .post(url)
            .headers(headers.clone())
            .json(body)
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

/// 处理 `response.output_text.delta` 事件，提取文本增量并发送。
fn handle_response_delta(data: &str, tx: &mpsc::UnboundedSender<StreamEvent>) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data)
        && let Some(delta) = json["delta"].as_str() {
            let _ = tx.send(StreamEvent::Chunk(delta.to_string()));
        }
}

/// 处理 `response.reasoning.delta` 事件，提取推理增量并发送。
fn handle_reasoning_delta(data: &str, tx: &mpsc::UnboundedSender<StreamEvent>) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data)
        && let Some(delta) = json["delta"].as_str() {
            let _ = tx.send(StreamEvent::ReasoningChunk(delta.to_string()));
        }
}

/// 处理 `response.error` 事件，提取错误消息并发送。
fn handle_response_error(data: &str, tx: &mpsc::UnboundedSender<StreamEvent>) {
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
}

/// 处理文本增量类事件（delta），返回 `Some(should_continue)`。
#[inline]
fn handle_delta_event(event_type: &str, data: &str, tx: &mpsc::UnboundedSender<StreamEvent>) -> Option<bool> {
    match event_type {
        "response.output_text.delta" => {
            handle_response_delta(data, tx);
            Some(false)
        }
        "response.reasoning.delta" => {
            handle_reasoning_delta(data, tx);
            Some(false)
        }
        _ => None,
    }
}

/// 处理 Responses API 的具名 SSE 事件数据。
///
/// 返回 `true` 表示流应终止（收到完成或错误事件），`false` 表示继续。
#[inline]
pub fn handle_responses_event(
    event_type: &str,
    data: &str,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> bool {
    if let Some(result) = handle_delta_event(event_type, data, tx) {
        return result;
    }
    match event_type {
        "response.completed" => {
            let _ = tx.send(StreamEvent::Complete);
            true
        }
        "response.error" => {
            handle_response_error(data, tx);
            true
        }
        _ => false,
    }
}

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
        let body = build_request_body(&params);

        if let Err(app_err) = resolve_and_send(client, &url, &params, &body, &tx).await {
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
    body: &serde_json::Value,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> Result<(), AppError> {
    let headers = resolve_auth_headers(params).ok_or(AppError::Auth {
        reason: AuthFailReason::InvalidKeyFormat,
    })?;

    let response = send_with_retry(client, url, &headers, body, 3).await
        .map_err(|e| AppError::Network { detail: e.to_string() })?;

    if !response.status().is_success() {
        handle_error_response_no_consume(response, tx).await;
        return Ok(());
    }

    parse_responses_stream(response, tx.clone()).await;
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

/// 解析 SSE `data:` 行内容，返回 `true` 表示流应终止。
fn parse_sse_data_line(
    data: &str,
    current_event: &mut String,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> bool {
    if data == "[DONE]" {
        let _ = tx.send(StreamEvent::Complete);
        return true;
    }

    if handle_responses_event(current_event, data, tx) {
        return true;
    }
    current_event.clear();
    false
}

/// 处理 SSE 行解析，返回 `true` 表示流应终止。
///
/// 根据行前缀分发处理：`event:` 设置当前事件类型，`data:` 解析事件数据，
/// 空行重置事件类型，不完整行保留到 buffer。
#[inline]
pub fn process_sse_line(
    line: &str,
    current_event: &mut String,
    tx: &mpsc::UnboundedSender<StreamEvent>,
    buffer: &mut String,
    is_last_line: bool,
) -> bool {
    let trimmed = line.trim();

    if let Some(event_type) = trimmed.strip_prefix("event:") {
        *current_event = event_type.trim().to_string();
    } else if let Some(data) = trimmed.strip_prefix("data:") {
        return parse_sse_data_line(data.trim(), current_event, tx);
    } else if trimmed.is_empty() {
        // 空行分隔事件，重置 event 类型
        current_event.clear();
    } else if is_last_line {
        // 不完整行，保留到下次
        buffer.push_str(trimmed);
        buffer.push('\n');
    }
    false
}

/// 处理 OpenAI Responses API 流式响应的单个字节块。
///
/// 将字节块追加到缓冲区，逐行解析 SSE 事件，返回 `true` 表示流应终止。
pub fn process_responses_chunk(
    chunk: &[u8],
    buffer: &mut String,
    current_event: &mut String,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> bool {
    buffer.push_str(&String::from_utf8_lossy(chunk));

    let lines: Vec<String> = buffer.lines().map(|l| l.to_owned()).collect();
    buffer.clear();

    for (i, line) in lines.iter().enumerate() {
        let is_last = i == lines.len() - 1;
        if process_sse_line(line, current_event, tx, buffer, is_last) {
            return true;
        }
    }
    false
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
                if process_responses_chunk(&chunk, &mut buffer, &mut current_event, &tx) {
                    return;
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
