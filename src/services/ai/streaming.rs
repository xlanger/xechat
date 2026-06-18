//! SSE 流式响应处理工具。
//!
//! 提供 Server-Sent Events 解析、token 估算、上下文压缩、错误消息提取等纯函数。
//! 本模块属于 services 层，依赖 `crate::models::ai` 提供的数据类型。

use crate::models::ai::{ChatMessage, StreamEvent};
use crate::models::error::AppError;
use futures_util::StreamExt;
use reqwest::Response;
use tokio::sync::mpsc;

const MIN_KEEP_MESSAGES: usize = 4;

/// 最大上下文消息条数（硬上限），超过则强制截断。
const MAX_CONTEXT_MESSAGES: usize = 20;

/// 估算文本对应的 token 数量。
///
/// 基于字符数除以 3.5 的近似算法：英文约 4 字符/token，中文约 1-2 字符/token。
///
/// # Arguments
///
/// * `text` - 待估算的文本
///
/// # Returns
///
/// 估算的 token 数量，空字符串返回 0。
pub fn estimate_tokens(text: &str) -> usize {
    let char_count = text.chars().count();
    if char_count == 0 {
        return 0;
    }
    (char_count as f64 / 3.5).ceil() as usize
}

/// 按消息条数截断（硬上限）。
fn truncate_by_count(messages: &[ChatMessage], max_count: usize) -> Vec<ChatMessage> {
    if messages.len() > max_count {
        messages[messages.len() - max_count..].to_vec()
    } else {
        messages.to_vec()
    }
}

/// 按 token 数截断，保留至少 MIN_KEEP_MESSAGES 条。
fn truncate_by_tokens(messages: &[ChatMessage], max_tokens: usize) -> Vec<ChatMessage> {
    let mut selected = messages.to_vec();
    while selected.len() > MIN_KEEP_MESSAGES {
        let current_total: usize = selected.iter().map(|m| estimate_tokens(&m.content)).sum();
        if current_total <= max_tokens {
            break;
        }
        selected.remove(0);
    }
    selected
}

/// 压缩对话消息列表以适配模型上下文窗口。
///
/// 当消息总 token 数超过 `max_tokens` 时，从头部逐条移除旧消息，
/// 直至满足限制或达到最小保留条数（4 条）。
///
/// # Arguments
///
/// * `messages` - 原始对话消息列表
/// * `max_tokens` - 模型允许的最大上下文 token 数
/// * `auto_management` - 是否启用自动压缩；关闭时原样返回
///
/// # Returns
///
/// 压缩后的消息副本。当 `auto_management` 为 false 或消息为空时原样返回。
pub fn compress_messages(
    messages: &[ChatMessage],
    max_tokens: u32,
    auto_management: bool,
) -> Vec<ChatMessage> {
    if !auto_management || messages.is_empty() {
        return messages.to_vec();
    }

    let selected = truncate_by_count(messages, MAX_CONTEXT_MESSAGES);
    truncate_by_tokens(&selected, max_tokens as usize)
}

/// 从 HTTP 错误响应体中提取可读的错误消息。
///
/// 尝试解析 OpenAI 兼容格式 `{"error":{"message":"..."}}` 的 JSON 结构。
///
/// # Arguments
///
/// * `body` - HTTP 响应体文本
///
/// # Returns
///
/// 解析成功返回 `Some(message)`，否则返回 `None`。
pub fn extract_error_from_body(body: &str) -> Option<String> {
    if let Some(pos) = body.find("{\"error\":") {
        let slice = &body[pos..];
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(slice)
            && let Some(msg) = parsed["error"]["message"].as_str() {
                return Some(msg.to_string());
            }
    }
    None
}

/// 从 SSE 行中提取 `data: ` 前缀后的数据字段。
///
/// # Arguments
///
/// * `line` - SSE 流中的一行文本
///
/// # Returns
///
/// 若行以 `data: ` 开头则返回 `Some(数据内容)`，否则返回 `None`。
pub fn extract_data_field(line: &str) -> Option<&str> {
    line.strip_prefix("data: ")
}

/// 判断 SSE 行是否为元数据行（event:/id:/retry:）或空行。
pub fn is_sse_metadata_or_empty(line: &str) -> bool {
    line.starts_with("event:")
        || line.starts_with("id:")
        || line.starts_with("retry:")
        || line.is_empty()
}

/// 解析 ChatResponse 中的 delta 字段，发送 Chunk 和 ReasoningChunk 事件。
fn handle_chat_response_delta(
    resp: &crate::models::ai::ChatResponse,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) {
    if let Some(delta) = resp.choices.first().and_then(|c| c.delta.as_ref()) {
        if let Some(content) = delta.content.as_ref() {
            let _ = tx.send(StreamEvent::Chunk(content.clone()));
        }
        if let Some(reasoning) = delta.reasoning_content.as_ref() {
            let _ = tx.send(StreamEvent::ReasoningChunk(reasoning.clone()));
        }
    }
}

/// 尝试从 JSON 响应中提取错误消息并发送 Error 事件。
///
/// 返回 `true` 如果找到错误（流应终止），`false` 如果不是错误响应。
fn try_handle_error_response(data: &str, tx: &mpsc::UnboundedSender<StreamEvent>) -> bool {
    if let Ok(err_resp) = serde_json::from_str::<serde_json::Value>(data) {
        if let Some(error_msg) = err_resp["error"]["message"].as_str() {
            let _ = tx.send(StreamEvent::Error(AppError::Api {
                status: 0,
                body: Some(error_msg.to_string()),
            }));
            return true;
        }
    }
    false
}

/// 处理 SSE data 字段内容，解析为 `StreamEvent` 并通过 channel 推送。
///
/// 遇到 `[DONE]` 或错误响应时返回 `true`，表示流应终止。
pub fn handle_sse_data(data: &str, tx: &mpsc::UnboundedSender<StreamEvent>) -> bool {
    use crate::models::ai::ChatResponse;

    if data == "[DONE]" {
        let _ = tx.send(StreamEvent::Complete);
        return true;
    }

    if let Ok(resp) = serde_json::from_str::<ChatResponse>(data) {
        handle_chat_response_delta(&resp, tx);
        return false;
    }

    try_handle_error_response(data, tx)
}

/// 处理 SSE 缓冲区中的完整行，返回是否应终止流。
///
/// 遍历缓冲区中的所有行，对 `data: ` 行调用 `handle_sse_data`，
/// 将不完整的最后一行保留在缓冲区中。
///
/// # Arguments
///
/// * `lines` - 按换行符分割的行列表
/// * `tx` - 用于推送 StreamEvent 的 channel 发送端
/// * `buffer` - 不完整行的缓冲区（会被清空后重填）
///
/// # Returns
///
/// 若遇到 `[DONE]` 或错误响应返回 `true`，表示流应终止。
pub fn process_sse_lines(
    lines: &[String],
    tx: &mpsc::UnboundedSender<StreamEvent>,
    buffer: &mut String,
) -> bool {
    buffer.clear();
    for (i, line) in lines.iter().enumerate() {
        if process_single_sse_line(line, i, lines.len(), tx, buffer) {
            return true;
        }
    }
    false
}

/// 处理单行 SSE 数据。
///
/// 返回 `true` 表示流应终止。
fn process_single_sse_line(
    line: &str,
    index: usize,
    total_lines: usize,
    tx: &mpsc::UnboundedSender<StreamEvent>,
    buffer: &mut String,
) -> bool {
    if let Some(data) = extract_data_field(line) {
        return handle_sse_data(data, tx);
    }
    if !is_sse_metadata_or_empty(line) && index == total_lines - 1 {
        buffer.push_str(line);
        buffer.push('\n');
    }
    false
}

/// 处理 SSE 流中的一个数据块：追加到缓冲区并处理完整行。
///
/// 返回 `true` 表示流应终止（遇到 [DONE] 或错误）。
pub fn process_stream_chunk(
    chunk: &[u8],
    buffer: &mut String,
    tx: &mpsc::UnboundedSender<StreamEvent>,
) -> bool {
    buffer.push_str(&String::from_utf8_lossy(chunk));
    let lines: Vec<String> = buffer.lines().map(|l| l.to_owned()).collect();
    process_sse_lines(&lines, tx, buffer)
}

/// 解析 SSE (Server-Sent Events) 流式响应。
///
/// 从 HTTP Response 的字节流中逐行读取 SSE 数据行，
/// 解析 `data: ` 前缀的 JSON 并通过 channel 推送 `StreamEvent`。
///
/// 遇到 `data: [DONE]` 标记流结束，任何读取错误也会终止解析并推送 Error 事件。
///
/// # Arguments
///
/// * `response` - 已建立的 HTTP 流式响应
/// * `tx` - 用于推送 StreamEvent 的 channel 发送端
pub async fn parse_sse_stream(
    response: Response,
    tx: mpsc::UnboundedSender<StreamEvent>,
) {
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                if process_stream_chunk(&chunk, &mut buffer, &tx) {
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

