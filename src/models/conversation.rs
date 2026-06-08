//! 对话会话数据模型。
//!
//! 本模块定义了 [`Conversation`] 结构体，代表一次完整的聊天会话。
//! 每个会话包含一个唯一的 ID、标题、消息列表以及创建/更新时间戳。
//! 会话数据通过 JSON 序列化/反序列化进行持久化存储。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::Message;

/// 一次完整的聊天对话会话。
///
/// 包含会话元数据（ID、标题、时间戳）和该会话下的所有消息。
/// 通过 [`Conversation::new()`] 构造新会话时自动生成 UUID 和时间戳。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Conversation {
    /// 会话唯一标识符（UUID v4）
    pub id: String,
    /// 会话标题，显示在侧边栏会话列表中
    pub title: String,
    /// 该会话下的所有消息列表，按发送时间顺序排列
    pub messages: Vec<Message>,
    /// 会话创建时间（UTC）
    pub created_at: DateTime<Utc>,
    /// 会话最后更新时间（UTC），每次添加新消息时更新
    pub updated_at: DateTime<Utc>,
    /// 是否为临时对话（内存-only，不持久化）
    #[serde(skip)]
    pub is_temporary: bool,
}

impl Conversation {
    /// 创建一个新的对话会话。
    ///
    /// 自动生成 UUID v4 作为会话标识符，并设置创建时间与更新时间
    /// 为当前 UTC 时间。消息列表初始为空。
    ///
    /// # Arguments
    ///
    /// * `title` - 会话标题，用于在侧边栏中显示
    ///
    /// # Returns
    ///
    /// 初始化完成的新 [`Conversation`] 实例。
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            is_temporary: false,
        }
    }

    /// 创建一个临时对话（内存-only，不持久化）。
    ///
    /// 临时对话在发送第一条消息前不会写入磁盘，
    /// 切换走或关闭应用时自动丢弃。
    pub fn new_temporary(title: String) -> Self {
        let mut conv = Self::new(title);
        conv.is_temporary = true;
        conv
    }
}
