//! AI 交互模块入口。
//!
//! 提供统一的 `send_message()` 函数，根据 `provider_key` 自动路由到
//! 对应的 Provider 实现（目前支持 OpenAI 兼容协议和 Ollama 协议）。
//! 本模块属于 services 层，依赖 `crate::models::ai` 的数据类型。

pub mod streaming;
pub mod providers;

pub use providers::*;
pub use streaming::{parse_sse_stream, compress_messages, estimate_tokens, extract_error_from_body, extract_data_field, is_sse_metadata_or_empty, handle_sse_data};

use crate::models::ai::{SendMessageParams, StreamEvent, AiProvider};
use reqwest::Client;
use tokio::sync::mpsc;

/// 统一 AI 消息发送入口。
///
/// 根据提供商标识符，将请求分发到对应的 Provider 实现。
/// 所有结果通过 `StreamEvent` channel 异步推送。
///
/// # Arguments
///
/// * `client` - HTTP 客户端实例
/// * `params` - 发送参数，包含 base_url / api_key / model / messages 等
/// * `provider_key` - 提供商标识符，如 `"deepseek"`、`"ollama"`、`"openai-compatible"`
/// * `tx` - 用于推送 StreamEvent 的 channel 发送端
///
/// # Errors
///
/// 通过 `StreamEvent::Error` 推送 [`AppError`]：
/// - [`AppError::Unsupported`] — 不支持的协议类型
/// - 底层 Provider 执行失败（网络错误、认证失败、服务端错误等）
pub async fn send_message(
    client: &Client,
    params: SendMessageParams,
    provider_key: &str,
    tx: mpsc::UnboundedSender<StreamEvent>,
) {
    match provider_key {
        "deepseek" => {
            providers::DeepSeekProvider
                .send_stream(client, params, tx)
                .await
        }
        "openai" => {
            providers::OpenAiProvider
                .send_stream(client, params, tx)
                .await
        }
        "ollama" => {
            providers::OllamaProvider
                .send_stream(client, params, tx)
                .await
        }
        _ => {
            providers::OpenAiCompatibleProvider
                .send_stream(client, params, tx)
                .await
        }
    }
}
