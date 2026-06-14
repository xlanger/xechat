//! 消息数据模型及状态/角色枚举。
//!
//! 本模块定义了聊天消息的核心数据结构 `Message`，
//! 以及消息角色枚举 `MessageRole` 和消息状态枚举 `MessageStatus`。
//! 消息通过 JSON 序列化/反序列化进行持久化存储。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 消息的发送/处理状态。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MessageStatus {
    /// 消息正在发送中（等待服务端响应）
    Sending,
    /// 消息已成功发送并接收完整响应
    Sent,
    /// 消息发送失败
    Failed,
    /// 用户手动停止流式接收，内容已截断
    Truncated,
}

/// 消息的发送者角色。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    /// 用户发送的消息
    User,
    /// AI 助手返回的消息
    Assistant,
}

/// 单条聊天消息，包含角色、内容、状态和时间戳。
///
/// 通过 [`Message::new_user()`] 和 [`Message::new_assistant()`] 两个构造方法
/// 分别创建用户消息和 AI 助手消息，自动生成 UUID 和时间戳。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// 消息唯一标识符（UUID v4）
    pub id: String,
    /// 消息发送者角色（用户或助手）
    pub role: MessageRole,
    /// 消息文本内容
    pub content: String,
    /// 推理过程文本（如模型支持的 reasoning 字段），仅 Assistant 消息可能有值
    pub reasoning_content: Option<String>,
    /// 消息创建时间（UTC）
    pub timestamp: DateTime<Utc>,
    /// 消息当前状态（发送中/已发送/失败/截断）
    pub status: MessageStatus,
}

impl Message {
    /// 创建一条用户消息。
    ///
    /// 自动设置角色为 [`MessageRole::User`]、状态为 [`MessageStatus::Sent`]，
    /// 生成 UUID v4 标识符和时间戳。
    ///
    /// # Arguments
    ///
    /// * `content` - 用户输入的文本内容
    ///
    /// # Returns
    ///
    /// 初始化完成的用户消息实例。
    pub fn new_user(content: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content,
            reasoning_content: None,
            timestamp: Utc::now(),
            status: MessageStatus::Sent,
        }
    }

    /// 创建一条 AI 助手消息（初始状态为空内容、发送中）。
    ///
    /// 自动设置角色为 [`MessageRole::Assistant`]、状态为 [`MessageStatus::Sending`]，
    /// 内容初始为空字符串，后续通过流式事件逐步填充。
    ///
    /// # Returns
    ///
    /// 初始化完成的空助手消息实例。
    pub fn new_assistant() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            reasoning_content: None,
            timestamp: Utc::now(),
            status: MessageStatus::Sending,
        }
    }

    /// 创建一条带内容的 AI 助手消息（状态为已发送）。
    ///
    /// 自动设置角色为 [`MessageRole::Assistant`]、状态为 [`MessageStatus::Sent`]，
    /// 用于流式回复完成后直接创建完整消息。
    ///
    /// # Arguments
    ///
    /// * `content` - 助手回复的文本内容
    ///
    /// # Returns
    ///
    /// 初始化完成的助手消息实例。
    pub fn new_assistant_with_content(content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: content.to_string(),
            reasoning_content: None,
            timestamp: Utc::now(),
            status: MessageStatus::Sent,
        }
    }
}
