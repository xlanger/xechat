//! AI 对话数据模型与流式接口定义。
//!
//! 本模块定义了与 AI 提供商交互所需的所有数据结构（DTO）和 trait 签名，
//! 属于 **models 层**，零 UI/I/O 依赖。具体实现在 `services::ai` 中完成。
//!
//! 包含内容：
//! - [`ChatMessage`] / [`ChatRequest`] / [`ChatResponse`] — OpenAI 兼容协议的请求/响应结构
//! - [`StreamEvent`] — 流式事件枚举（Chunk / Complete / Error）
//! - [`SendMessageParams`] — 发送消息的完整参数
//! - [`AiProvider`] — AI 提供商抽象 trait
//! - 默认常量：[`DEFAULT_MAX_CONTEXT_TOKENS`] / [`DEFAULT_AUTO_CONTEXT_MANAGEMENT`]

use serde::{Deserialize, Serialize};
use super::config::ModelConfig;
use super::config::ModelProvider;
use super::error::AppError;
use reqwest::Client;
use tokio::sync::mpsc;

/// 单条对话消息，用于构造 API 请求体。
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    /// 消息角色标识（如 `"system"`、`"user"`、`"assistant"`）
    pub role: String,
    /// 消息文本内容
    pub content: String,
}

/// 聊天补全请求体，对应 OpenAI `/v1/chat/completions` 接口格式。
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    /// 目标模型标识符（如 `"deepseek-v4-flash"`）
    pub model: String,
    /// 对话消息列表，包含系统提示词和历史消息
    pub messages: Vec<ChatMessage>,
    /// 是否启用流式输出
    pub stream: bool,
    /// 采样温度 (0.0–2.0)，值越高输出越随机；为 `None` 时由服务端决定
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// 核采样阈值 (0.0–1.0)，与 temperature 二选一使用；为 `None` 时不传
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// 模型最大输出 token 数，为 `None` 时不传
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// 频率惩罚 (0.0–2.0)，为 `None` 时不传
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// 存在惩罚 (0.0–2.0)，为 `None` 时不传
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// 自定义停止序列，为空时不传
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
}

/// 聊天补全响应体，包含模型生成的选择结果。
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    /// 模型返回的选择列表（通常只有一个元素）
    pub choices: Vec<ChatChoice>,
}

/// 模型单次选择的增量数据。
///
/// 在流式模式下仅填充 [`delta`] 字段，非流式模式下可能包含完整消息。
///
/// [`delta`]: ChatChoice::delta
#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    /// 增量内容片段，流式模式下每次 SSE 事件携带一段文本
    pub delta: Option<ChatDelta>,
}

/// 单次 SSE 推送的文本增量。
#[derive(Debug, Deserialize)]
pub struct ChatDelta {
    /// 本次推送的文本片段，`None` 表示该事件无内容（如结束标记）
    pub content: Option<String>,
    /// 推理过程文本片段（如模型支持的 reasoning 字段）
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

/// 流式传输过程中产生的事件类型。
///
/// 通过 `mpsc::UnboundedSender<StreamEvent>` channel 从 services 层推送到 stores 层。
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// 文本块：模型生成的一段增量内容
    Chunk(String),
    /// 推理过程文本块：模型的思考/推理过程增量内容
    ReasoningChunk(String),
    /// 流式传输正常结束，表示本次响应已完整接收
    Complete,
    /// 错误：携带结构化的应用错误信息（含错误域分类和上下文）
    Error(AppError),
}

/// 发送消息到 AI 提供商所需的全部参数。
///
/// 由 stores 层组装后传入 services 层，用于构造 HTTP 请求。
#[derive(Debug, Clone)]
pub struct SendMessageParams {
    /// Provider 配置（包含 base_url、api_key 等）
    pub provider: ModelProvider,
    /// Provider 标识符（如 `"deepseek"`），用于环境变量回退
    pub provider_key: String,
    /// 目标模型标识符
    pub model: String,
    /// 完整对话上下文消息列表（含系统提示词）
    pub messages: Vec<ChatMessage>,
    /// 采样温度 (0.0–2.0)，`None` 表示不指定
    pub temperature: Option<f32>,
    /// 核采样阈值 (0.0–1.0)，`None` 表示不指定
    pub top_p: Option<f32>,
    /// 当前模型的完整参数配置，供 Provider 读取扩展参数
    pub model_config: Option<ModelConfig>,
}

/// AI 提供商抽象 trait，定义流式发送接口。
///
/// 实现者需处理 HTTP 请求构造、SSE 流解析、错误转换，
/// 并通过 [`StreamEvent`] channel 向调用方推送结果。
///
/// # Example
///
/// ```ignore
/// use crate::models::ai::{AiProvider, SendMessageParams, StreamEvent};
/// use reqwest::Client;
/// use tokio::sync::mpsc;
///
/// struct MyProvider;
///
/// impl AiProvider for MyProvider {
///     async fn send_stream(
///         &self,
///         client: &Client,
///         params: SendMessageParams,
///         tx: mpsc::UnboundedSender<StreamEvent>,
///     ) {
///         // 构造 HTTP 请求 → 解析 SSE → 通过 tx 推送 StreamEvent
///     }
/// }
/// ```
#[allow(async_fn_in_trait)]
pub trait AiProvider: Send + Sync {
    /// 向 AI 服务端发起流式请求并持续推送结果。
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP 客户端实例，复用以利用连接池
    /// * `params` - 发送参数，包含 base_url / api_key / model / messages 等
    /// * `tx` - 无界 channel 发送端，用于向调用方推送 [`StreamEvent`]
    async fn send_stream(
        &self,
        client: &Client,
        params: SendMessageParams,
        tx: mpsc::UnboundedSender<StreamEvent>,
    );
}

/// 默认最大上下文 token 数量（8K），用于控制发送给模型的上下文窗口大小。
pub const DEFAULT_MAX_CONTEXT_TOKENS: u32 = 8192;

/// 默认最大上下文消息条数，超过则从头部移除旧消息。
pub const DEFAULT_MAX_CONTEXT_MESSAGES: usize = 20;

/// 是否默认启用自动上下文管理（自动截断/压缩超长对话历史）。
pub const DEFAULT_AUTO_CONTEXT_MANAGEMENT: bool = true;
