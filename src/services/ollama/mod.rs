//! Ollama 服务集成模块。
//!
//! 提供 Ollama 服务的自动探测、模型分类与状态管理。
//! - `probe`：探测 Ollama 服务并分类模型（嵌入/聊天）

pub mod embed;
pub mod probe;

use serde::{Deserialize, Serialize};

/// Ollama 服务运行时状态。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OllamaStatus {
    /// Ollama 服务地址
    pub host: String,
    /// 服务是否可用
    pub available: bool,
    /// Ollama 版本号
    pub version: String,
    /// 自动检测到的嵌入模型
    pub embed_model: Option<String>,
    /// 自动检测到的聊天模型
    pub chat_model: Option<String>,
}

/// Ollama 服务配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Ollama 服务地址
    pub host: String,
    /// 用户偏好的嵌入模型（覆盖自动检测）
    pub preferred_embed: Option<String>,
    /// 用户偏好的聊天模型（覆盖自动检测）
    pub preferred_chat: Option<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: "http://localhost:11434".to_string(),
            preferred_embed: None,
            preferred_chat: None,
        }
    }
}

impl OllamaConfig {
    /// 从应用配置构建 OllamaConfig。
    ///
    /// 仅当对应的 provider 为 "ollama" 且模型名非空时，才设置 preferred_embed。
    /// embed 判断使用 `preferences.embed_provider`。
    pub fn from_app_config(config: &crate::models::config::XEChatConfig) -> Self {
        Self {
            host: if config.preferences.ollama.host.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                config.preferences.ollama.host.clone()
            },
            preferred_embed: if config.preferences.embed_provider == "ollama" && !config.preferences.ollama.embed_model.is_empty() {
                Some(config.preferences.ollama.embed_model.clone())
            } else {
                None
            },
            preferred_chat: None,
        }
    }
}
