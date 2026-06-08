use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// LanceDB 向量搜索结果。
///
/// 包含相似度分数和完整的元数据字段，
/// 用于语义搜索结果展示和记忆管线上下文组装。
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// 相似度分数（1 - cosine distance），范围 [0, 1]。
    pub score: f32,
    /// LanceDB 记忆条目 ID。
    pub entry_id: String,
    /// 所属对话 ID。
    pub conversation_id: String,
    /// 所属消息 ID。
    pub message_id: String,
    /// 消息内容。
    pub content: String,
    /// 消息角色（user / assistant）。
    pub role: String,
    /// 消息时间戳（RFC 3339 格式）。
    pub timestamp: String,
    /// 用户消息 ID（轮次聚合检索时填充）
    pub user_message_id: String,
    /// 用户消息原文（轮次聚合检索时填充）
    pub user_content: String,
    /// 分块序号（-1 表示未分块/整条轮次）
    pub chunk_index: i32,
}

/// 对话轮次条目，用于向量存储。
///
/// 一个轮次 = 一条用户消息 + 一条助手回复，
/// 构成语义完整的问答对。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnEntry {
    /// 轮次唯一 ID
    pub id: String,
    /// 所属对话 ID
    pub conversation_id: String,
    /// 用户消息 ID
    pub user_message_id: String,
    /// 助手消息 ID
    pub assistant_message_id: String,
    /// 轮次序号（0-based，对话内递增）
    pub turn_index: u32,
    /// 用户消息原文
    pub user_content: String,
    /// 助手回复原文
    pub assistant_content: String,
    /// 轮次时间戳（取助手回复完成时间）
    pub timestamp: DateTime<Utc>,
    /// 分块元数据（单块时长度为 1）
    pub chunks: Vec<ChunkMeta>,
}

/// 分块元数据，描述轮次内的一个文本分块。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkMeta {
    /// 分块序号（0-based）
    pub chunk_index: u32,
    /// 分块文本（含前缀的轮次合并文本的子段）
    pub chunk_text: String,
    /// 在轮次合并文本中的起始字符偏移
    pub start_char: u32,
    /// 在轮次合并文本中的结束字符偏移
    pub end_char: u32,
    /// 分块嵌入向量
    #[serde(skip)]
    pub embedding: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntentResult {
    pub needs_memory: bool,
    pub confidence: f32,
    pub memory_query: String,
    pub time_hint: TimeRange,
    pub action: IntentAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntentAction {
    DirectQuery,
    SimpleContext,
    MemoryRetrieve,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeRange {
    Any,
    RecentDays(u32),
    SpecificMonth(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchType {
    FullText,
    Semantic,
    Hybrid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub conversation_id: String,
    pub message_id: String,
    pub title: String,
    pub content_snippet: String,
    pub role: String,
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub search_type: SearchType,
    /// 对话创建时间。
    pub created_at: DateTime<Utc>,
    /// 对话消息条数。
    pub message_count: usize,
    /// 最新一条助手消息摘要（截断至约 80 字符）。
    pub last_assistant_snippet: String,
}
