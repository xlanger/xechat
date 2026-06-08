//! 数据模型层（Models Layer），五层架构中的核心数据结构层。
//!
//! 本模块定义了 XEChat 全部业务数据结构、trait 抽象和常量。
//! 作为五层架构的第二层，models 层遵循**零 UI 依赖、零 I/O 依赖**原则，
//! 只包含纯数据定义和与上层（stores）交互的 trait 签名。
//!
//! # 子模块职责
//!
//! | 子模块 | 职责 |
//! |--------|------|
//! | [`ai`] | AI 对话数据模型、流式事件类型、AI 提供商 trait 及默认常量 |
//! | [`config`] | 应用配置数据结构（提供商、模型参数） |
//! | [`conversation`] | 对话会话数据结构 |
//! | [`message`] | 消息数据结构及状态/角色枚举 |
//! | [`error`] | 统一错误类型及错误域分类 |
//! | [`i18n`] | 国际化翻译包数据结构 |
//!
//! # 在五层架构中的位置
//!
//! ```text
//! UI (components) → Stores → Models → Services → External APIs
//!                        ↑
//!                   本层在此
//! ```
//!
//! stores 层使用本模块的类型进行状态管理，services 层依赖本模块的 trait
//! 进行具体实现。本模块不依赖 UI 框架和文件系统操作。

pub mod conversation;
pub mod config;
pub mod message;
pub mod ai;
pub mod error;
pub mod i18n;
pub mod memory;

pub use conversation::Conversation;
pub use config::{XEChatConfig, ModelProvider, ModelConfig, MemoryConfig, PreferencesConfig, OllamaPreferences};
pub use message::{Message, MessageRole, MessageStatus};
pub use ai::{ChatMessage, ChatRequest, ChatResponse, ChatChoice, ChatDelta, StreamEvent, SendMessageParams, AiProvider, DEFAULT_MAX_CONTEXT_TOKENS, DEFAULT_AUTO_CONTEXT_MANAGEMENT, DEFAULT_MAX_CONTEXT_MESSAGES};
pub use error::{AppError, AuthFailReason};
pub use i18n::Language;
pub use memory::{SearchHit, TurnEntry, ChunkMeta};
