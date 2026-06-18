//! AI Provider 实现模块。
//!
//! 每个子模块实现一种后端协议的 [`crate::models::ai::AiProvider`] trait。
//! 当前支持的 Provider：
//! - `deepseek`：DeepSeek Chat Completions 协议（SSE 流式，含 reasoning_content）
//! - `ollama`：Ollama 本地推理（NDJSON 流式）
//! - `openai`：OpenAI Responses API（SSE 具名事件流式）
//! - `openai_compatible`：OpenAI 兼容协议（通用中转服务）
//!
//! `retry` 子模块提供 4 个 Provider 共用的 HTTP 重试逻辑。
//!
//! 新增 Provider 只需引入子模块并在此 `pub use` 导出即可。

pub mod deepseek;
pub mod ollama;
pub mod openai;
pub mod openai_compatible;
pub mod retry;

pub use deepseek::DeepSeekProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
pub use openai_compatible::OpenAiCompatibleProvider;
