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

/// 构造 Ollama 请求体。
#[inline]
pub fn build_request_body(params: &SendMessageParams) -> serde_json::Value {
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

    serde_json::json!({
        "model": params.model,
        "messages": messages,
        "stream": true,
        "options": {
            "temperature": params.temperature.unwrap_or(0.7),
        }
    })
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
    body: &serde_json::Value,
    max_retries: u32,
) -> Result<reqwest::Response, reqwest::Error> {
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        match client
            .post(url)
            // 不设 .timeout()：它覆盖整个请求+响应体读取，
            // 对流式响应不合适（模型生成可能需要数分钟）。
            // 连接超时已在 Client::builder().connect_timeout() 中设置。
            .json(body)
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
    let fallback = extract_error_from_body(&body_text);
    let _ = tx.send(StreamEvent::Error(AppError::Api {
        status: status.as_u16(),
        body: fallback.or(if body_text.is_empty() {
            None
        } else {
            Some(body_text)
        }),
    }));
}

/// 解析 Ollama NDJSON 行中的单条 JSON 消息。
///
/// 返回 `true` 表示流应终止（收到 done 标记），`false` 表示继续。
#[inline]
pub fn handle_ollama_json_line(
    json: &serde_json::Value,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> bool {
    if json["done"].as_bool() == Some(true) {
        let _ = tx.send(StreamEvent::Complete);
        return true;
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
    false
}

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

        let body = build_request_body(&params);

        let response = match send_with_retry(client, &url, &body, 3).await {
            Ok(resp) => resp,
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(AppError::Network {
                    detail: e.to_string(),
                }));
                return;
            }
        };

        if !response.status().is_success() {
            handle_error_response(response, &tx).await;
            return;
        }

        parse_ollama_stream(response, tx).await;
    }
}

/// 处理 Ollama NDJSON 中的单行。
///
/// 返回 `true` 表示流应终止（收到 done 标记），`false` 表示继续。
/// 不完整的行（非最后一行且不以 `}` 结尾）追加到 `remaining` 缓冲区。
#[inline]
pub fn process_ollama_line(
    line: &str,
    is_last_line: bool,
    tx: &mpsc::UnboundedSender<StreamEvent>,
    remaining: &mut String,
) -> bool {
    let line = line.trim();
    if line.is_empty() {
        return false;
    }
    if is_last_line && !line.ends_with('}') {
        remaining.push_str(line);
        remaining.push('\n');
        return false;
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
        if handle_ollama_json_line(&json, tx) {
            return true;
        }
    }
    false
}

/// 处理 Ollama 流式响应的单个字节块。
///
/// 将字节块追加到缓冲区，逐行解析 NDJSON，返回 `true` 表示流应终止。
pub fn process_ollama_chunk(
    chunk: &[u8],
    buffer: &mut String,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> bool {
    buffer.push_str(&String::from_utf8_lossy(chunk));

    let lines: Vec<&str> = buffer.lines().collect();
    let mut remaining = String::new();

    for (i, line) in lines.iter().enumerate() {
        let is_last = i == lines.len() - 1;
        if process_ollama_line(line, is_last, tx, &mut remaining) {
            return true;
        }
    }
    *buffer = remaining;
    false
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
                if process_ollama_chunk(&chunk, &mut buffer, &tx) {
                    return;
                }
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
