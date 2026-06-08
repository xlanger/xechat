//! 基于 LanceDB 的对话持久化存储。
//!
//! 使用 LanceDB 的 `conversations` 表存储对话元数据和消息原文，
//! 替代文件系统 JSON 存储。提供对话 CRUD、消息追加/更新/删除等异步操作。
//! LanceDB 原生支持标量过滤（全文搜索）。

use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use lancedb::query::{ColumnOrdering, ExecutableQuery, QueryBase};
use lancedb::index::Index;
use once_cell::sync::OnceCell;

use crate::{Conversation, Message, MessageRole, MessageStatus};
use rust_i18n::t;

const TABLE_NAME: &str = "conversations";
/// 默认分页大小（对话消息瀑布流加载）。
pub const DEFAULT_PAGE_SIZE: usize = 10;
/// Sidebar 最大显示对话数。
pub const SIDEBAR_MAX_CONVERSATIONS: usize = 9;
/// 搜索分页大小。
pub const SEARCH_PAGE_SIZE: usize = 10;
/// 助手消息摘要最大字符数。
const ASSISTANT_SNIPPET_MAX_LEN: usize = 80;

/// 对话摘要（用于搜索页列表展示，不包含完整消息列表）。
#[derive(Debug, Clone)]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    /// 最新一条助手消息摘要。
    pub last_assistant_snippet: String,
}

impl From<ConversationSummary> for Conversation {
    fn from(s: ConversationSummary) -> Self {
        Conversation {
            id: s.id,
            title: s.title,
            messages: Vec::new(),
            created_at: s.created_at,
            updated_at: s.updated_at,
            is_temporary: false,
        }
    }
}

fn escape_sql(value: &str) -> String {
    value.replace('\'', "''")
}

/// 按字符截断字符串，避免 UTF-8 边界切割。
fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let end = s.char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}…", &s[..end])
    }
}

static STORE: OnceCell<ConversationStore> = OnceCell::new();

pub fn init_store(store: ConversationStore) -> anyhow::Result<()> {
    STORE.set(store).map_err(|_| anyhow::anyhow!("Store already initialized"))
}

pub fn get_store() -> Option<&'static ConversationStore> {
    STORE.get()
}

pub struct ConversationStore {
    db: lancedb::Connection,
    table: Option<lancedb::Table>,
    vector_store: Arc<dyn crate::services::vector_store::VectorStore>,
}

impl ConversationStore {
    pub async fn open(path: &str, vector_store: Arc<dyn crate::services::vector_store::VectorStore>) -> anyhow::Result<Self> {
        let db = lancedb::connect(path).execute().await?;
        let table = match db.open_table(TABLE_NAME).execute().await {
            Ok(t) => Some(t),
            Err(_) => None,
        };
        Ok(Self {
            db,
            table,
            vector_store,
        })
    }

    pub async fn ensure_table(&mut self) -> anyhow::Result<()> {
        if self.table.is_some() {
            // 表已存在，检查是否需要 schema 迁移
            self.migrate_schema().await?;
            return Ok(());
        }
        let schema = Self::arrow_schema();
        let batch = RecordBatch::new_empty(schema);
        let table = self
            .db
            .create_table(TABLE_NAME, vec![batch])
            .execute()
            .await?;

        // 创建全文搜索倒排索引（空表也可创建）
        if let Err(e) = table
            .create_index(&["content"], Index::FTS(Default::default()))
            .execute()
            .await
        {
            eprintln!("[xechat] Failed to create FTS index on conversations.content: {}", e);
        }

        self.table = Some(table);
        Ok(())
    }

    /// 检查并执行 schema 迁移，确保表包含所有必需列。
    ///
    /// LanceDB 支持通过 `add_columns` 添加新列。
    /// 新增列的默认值用 SQL 空字符串表达式填充。
    async fn migrate_schema(&mut self) -> anyhow::Result<()> {
        use lancedb::table::NewColumnTransform;

        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let schema = table.schema().await?;
        let existing_columns: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();

        // 需要迁移的列：(列名, SQL 默认值表达式)
        let required_columns: [(&str, &str); 1] = [
            ("reasoning_content", "''"),
        ];

        let missing: Vec<(String, String)> = required_columns
            .iter()
            .filter(|(col_name, _)| !existing_columns.iter().any(|ec| ec == *col_name))
            .map(|(col_name, default)| (col_name.to_string(), default.to_string()))
            .collect();

        if !missing.is_empty() {
            for (col_name, _) in &missing {
                eprintln!("[xechat] Migrating conversations table: adding column '{}'", col_name);
            }
            let count = missing.len();
            table.add_columns(NewColumnTransform::SqlExpressions(missing), None).await?;
            eprintln!("[xechat] Migration complete: {} columns added", count);
        }

        Ok(())
    }

    fn arrow_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("conversation_id", DataType::Utf8, false),
            Field::new("title", DataType::Utf8, false),
            Field::new("created_at", DataType::Utf8, false),
            Field::new("updated_at", DataType::Utf8, false),
            Field::new("message_id", DataType::Utf8, false),
            Field::new("role", DataType::Utf8, false),
            Field::new("content", DataType::Utf8, false),
            Field::new("reasoning_content", DataType::Utf8, false),
            Field::new("status", DataType::Utf8, false),
            Field::new("timestamp", DataType::Utf8, false),
        ]))
    }

    fn message_to_batch_limited(conv: &Conversation, max_msgs: usize) -> anyhow::Result<RecordBatch> {
        let schema = Self::arrow_schema();
        let msg_count = conv.messages.len().max(1).min(max_msgs);

        let conv_ids = StringArray::from_iter_values(
            std::iter::repeat(&conv.id).take(msg_count),
        );
        let titles = StringArray::from_iter_values(
            std::iter::repeat(&conv.title).take(msg_count),
        );
        let created_ats: Vec<String> = std::iter::repeat(conv.created_at.to_rfc3339())
            .take(msg_count)
            .collect();
        let updated_ats: Vec<String> = std::iter::repeat(conv.updated_at.to_rfc3339())
            .take(msg_count)
            .collect();

        let (msg_ids, roles, contents, reasoning_contents, statuses, timestamps): (
            Vec<String>, Vec<String>, Vec<String>, Vec<String>, Vec<String>, Vec<String>,
        ) = if conv.messages.is_empty() {
            (
                vec![format!("{}__empty", conv.id)],
                vec!["system".to_string()],
                vec![String::new()],
                vec![String::new()],
                vec!["Sent".to_string()],
                vec![conv.created_at.to_rfc3339()],
            )
        } else {
            let mut msg_ids = Vec::with_capacity(msg_count);
            let mut roles = Vec::with_capacity(msg_count);
            let mut contents = Vec::with_capacity(msg_count);
            let mut reasoning_contents = Vec::with_capacity(msg_count);
            let mut statuses = Vec::with_capacity(msg_count);
            let mut timestamps = Vec::with_capacity(msg_count);
            for m in conv.messages.iter().take(max_msgs) {
                msg_ids.push(m.id.clone());
                roles.push(match m.role {
                    MessageRole::User => "User".to_string(),
                    MessageRole::Assistant => "Assistant".to_string(),
                });
                contents.push(m.content.clone());
                reasoning_contents.push(m.reasoning_content.clone().unwrap_or_default());
                statuses.push(match m.status {
                    MessageStatus::Sending => "Sending".to_string(),
                    MessageStatus::Sent => "Sent".to_string(),
                    MessageStatus::Failed => "Failed".to_string(),
                    MessageStatus::Truncated => "Truncated".to_string(),
                });
                timestamps.push(m.timestamp.to_rfc3339());
            }
            (msg_ids, roles, contents, reasoning_contents, statuses, timestamps)
        };

        Ok(RecordBatch::try_new(schema, vec![
            Arc::new(conv_ids),
            Arc::new(titles),
            Arc::new(StringArray::from_iter_values(created_ats)),
            Arc::new(StringArray::from_iter_values(updated_ats)),
            Arc::new(StringArray::from_iter_values(msg_ids)),
            Arc::new(StringArray::from_iter_values(roles)),
            Arc::new(StringArray::from_iter_values(contents)),
            Arc::new(StringArray::from_iter_values(reasoning_contents)),
            Arc::new(StringArray::from_iter_values(statuses)),
            Arc::new(StringArray::from_iter_values(timestamps)),
        ])?)
    }

    fn message_to_batch(conv: &Conversation) -> anyhow::Result<RecordBatch> {
        Self::message_to_batch_limited(conv, usize::MAX)
    }

    fn batches_to_conversations(batches: &[RecordBatch]) -> Vec<Conversation> {
        use std::collections::{HashMap, hash_map::Entry};

        let mut conv_map: HashMap<String, Conversation> = HashMap::new();
        let mut temp_msg_map: HashMap<String, HashMap<String, Message>> = HashMap::new();

        for batch in batches {
            let conv_ids = batch.column_by_name("conversation_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let titles = batch.column_by_name("title")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let created_ats = batch.column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let updated_ats = batch.column_by_name("updated_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let msg_ids = batch.column_by_name("message_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let roles = batch.column_by_name("role")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let contents = batch.column_by_name("content")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let reasoning_contents = batch.column_by_name("reasoning_content")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let statuses = batch.column_by_name("status")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let timestamps = batch.column_by_name("timestamp")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let Some(conv_ids) = conv_ids else { continue };
            let row_count = conv_ids.len();

            for i in 0..row_count {
                let conv_id = conv_ids.value(i).to_string();

                let title = titles.map(|a| a.value(i)).unwrap_or("");
                let created_at = created_ats
                    .map(|a| DateTime::parse_from_rfc3339(a.value(i)).map(|dt| dt.with_timezone(&Utc)).unwrap_or_default())
                    .unwrap_or_default();
                let row_updated_at = updated_ats
                    .map(|a| DateTime::parse_from_rfc3339(a.value(i)).map(|dt| dt.with_timezone(&Utc)).unwrap_or_default())
                    .unwrap_or_default();

                let entry = conv_map.entry(conv_id.clone()).or_insert_with(|| Conversation {
                    id: conv_id.clone(),
                    title: title.to_string(),
                    messages: Vec::new(),
                    created_at,
                    updated_at: row_updated_at,
                    is_temporary: false,
                });

                if row_updated_at > entry.updated_at {
                    entry.updated_at = row_updated_at;
                    if !title.is_empty() {
                        entry.title = title.to_string();
                    }
                }

                let Some(msg_ids_arr) = msg_ids else { continue };
                let msg_id = msg_ids_arr.value(i).to_string();
                if msg_id.ends_with("__empty") {
                    continue;
                }

                let role = roles.map(|a| a.value(i)).unwrap_or("Assistant");
                let content = contents.map(|a| a.value(i)).unwrap_or("");
                let reasoning_content = reasoning_contents
                    .map(|a| a.value(i))
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());
                let status = statuses.map(|a| a.value(i)).unwrap_or("Sent");
                let timestamp = timestamps
                    .map(|a| DateTime::parse_from_rfc3339(a.value(i)).map(|dt| dt.with_timezone(&Utc)).unwrap_or_default())
                    .unwrap_or_default();

                let role = match role {
                    "User" => MessageRole::User,
                    _ => MessageRole::Assistant,
                };
                let status = match status {
                    "Sending" => MessageStatus::Sending,
                    "Failed" => MessageStatus::Failed,
                    "Truncated" => MessageStatus::Truncated,
                    _ => MessageStatus::Sent,
                };
                let content = content.to_string();

                let msg_map = temp_msg_map.entry(conv_id).or_insert_with(HashMap::new);
                match msg_map.entry(msg_id) {
                    Entry::Occupied(mut e) => {
                        let existing = e.get_mut();
                        if content.len() > existing.content.len() || timestamp > existing.timestamp {
                            existing.content = content;
                            existing.reasoning_content = reasoning_content;
                            existing.role = role;
                            existing.status = status;
                            existing.timestamp = timestamp;
                        }
                    }
                    Entry::Vacant(e) => {
                        let msg_id = e.key().clone();
                        e.insert(Message {
                            id: msg_id,
                            role,
                            content,
                            reasoning_content,
                            timestamp,
                            status,
                        });
                    }
                }
            }
        }

        for (conv_id, msg_map) in temp_msg_map {
            if let Some(entry) = conv_map.get_mut(&conv_id) {
                let mut messages: Vec<Message> = msg_map.into_values().collect();
                messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                entry.messages = messages;
            }
        }

        let mut convs: Vec<Conversation> = conv_map.into_values().collect();
        convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        convs
    }

    pub async fn create_conversation(&self, title: &str) -> anyhow::Result<Conversation> {
        let conv = Conversation::new(title.to_string());
        self.save_conversation(&conv).await?;
        Ok(conv)
    }

    pub async fn save_conversation(&self, conv: &Conversation) -> anyhow::Result<()> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let predicate = format!("conversation_id = '{}'", escape_sql(&conv.id));
        table.delete(&predicate).await?;

        let batch = Self::message_to_batch(conv)?;
        table.add(vec![batch]).execute().await?;

        Ok(())
    }

    async fn insert_message_row(
        &self,
        conv_id: &str,
        title: &str,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        message: &Message,
    ) -> anyhow::Result<()> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let schema = Self::arrow_schema();
        let batch = RecordBatch::try_new(schema.clone(), vec![
            Arc::new(StringArray::from_iter_values([conv_id])),
            Arc::new(StringArray::from_iter_values([title])),
            Arc::new(StringArray::from_iter_values([created_at.to_rfc3339()])),
            Arc::new(StringArray::from_iter_values([updated_at.to_rfc3339()])),
            Arc::new(StringArray::from_iter_values([&message.id])),
            Arc::new(StringArray::from_iter_values([match message.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
            }])),
            Arc::new(StringArray::from_iter_values([&message.content])),
            Arc::new(StringArray::from_iter_values([message.reasoning_content.as_deref().unwrap_or("")])),
            Arc::new(StringArray::from_iter_values([match message.status {
                MessageStatus::Sending => "Sending",
                MessageStatus::Sent => "Sent",
                MessageStatus::Failed => "Failed",
                MessageStatus::Truncated => "Truncated",
            }])),
            Arc::new(StringArray::from_iter_values([message.timestamp.to_rfc3339()])),
        ])?;

        table.add(vec![batch]).execute().await?;

        Ok(())
    }

    pub async fn load_conversation_by_id(&self, conv_id: &str, limit: usize) -> anyhow::Result<Option<Conversation>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table
            .query()
            .only_if(format!("conversation_id = '{}'", escape_sql(conv_id)))
            .order_by(Some(vec![ColumnOrdering::asc_nulls_last("timestamp".to_string())]))
            .limit(limit)
            .execute()
            .await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(None);
        }

        let convs = Self::batches_to_conversations(&batches);
        Ok(convs.into_iter().find(|c| c.id == conv_id))
    }

    async fn load_conversation_meta_by_id(&self, conv_id: &str) -> anyhow::Result<Option<Conversation>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table
            .query()
            .only_if(format!("conversation_id = '{}'", escape_sql(conv_id)))
            .limit(1)
            .execute()
            .await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(None);
        }

        let convs = Self::batches_to_conversations(&batches);
        Ok(convs.into_iter().find(|c| c.id == conv_id))
    }

    async fn load_message_by_id(&self, conv_id: &str, msg_id: &str) -> anyhow::Result<Option<Message>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table
            .query()
            .only_if(format!(
                "conversation_id = '{}' AND message_id = '{}'",
                escape_sql(conv_id),
                escape_sql(msg_id)
            ))
            .limit(1)
            .execute()
            .await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(None);
        }

        let convs = Self::batches_to_conversations(&batches);
        Ok(convs.into_iter().find(|c| c.id == conv_id).and_then(|c| c.messages.into_iter().find(|m| m.id == msg_id)))
    }

    async fn load_last_message(&self, conv_id: &str) -> anyhow::Result<Option<Message>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table
            .query()
            .only_if(format!("conversation_id = '{}'", escape_sql(conv_id)))
            .order_by(Some(vec![ColumnOrdering::desc_nulls_last("timestamp".to_string())]))
            .limit(1)
            .execute()
            .await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(None);
        }

        let convs = Self::batches_to_conversations(&batches);
        Ok(convs.into_iter().find(|c| c.id == conv_id).and_then(|c| c.messages.last().cloned()))
    }

    pub async fn load_conversation_messages_paged(
        &self,
        conv_id: &str,
        offset: usize,
        limit: usize,
    ) -> anyhow::Result<Vec<Message>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table
            .query()
            .only_if(format!("conversation_id = '{}'", escape_sql(conv_id)))
            .order_by(Some(vec![ColumnOrdering::asc_nulls_last("timestamp".to_string())]))
            .limit(limit)
            .offset(offset)
            .execute()
            .await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(Vec::new());
        }

        let convs = Self::batches_to_conversations(&batches);
        let messages = convs
            .into_iter()
            .find(|c| c.id == conv_id)
            .map(|c| c.messages)
            .unwrap_or_default();

        Ok(messages)
    }

    pub async fn load_conversation_list(&self, limit: usize) -> anyhow::Result<Vec<Conversation>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table
            .query()
            .order_by(Some(vec![ColumnOrdering::desc_nulls_last("updated_at".to_string())]))
            .limit(limit)
            .execute()
            .await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(Vec::new());
        }

        Ok(Self::batches_to_conversations(&batches))
    }

    /// 加载 sidebar 对话列表（按 updated_at 降序）。
    ///
    /// 仅提取 id / title / created_at / updated_at，不计算消息数和助手摘要。
    /// 供 sidebar 初始化使用，轻量高效。
    pub async fn load_sidebar_list(&self) -> anyhow::Result<Vec<Conversation>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table.query().execute().await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(Vec::new());
        }

        /// 最小聚合状态：只追踪对话元数据。
        struct SidebarState {
            title: String,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
        }

        let mut state_map: std::collections::HashMap<String, SidebarState> = std::collections::HashMap::new();

        for batch in &batches {
            let conv_ids = batch.column_by_name("conversation_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let titles = batch.column_by_name("title")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let created_ats = batch.column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let updated_ats = batch.column_by_name("updated_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let (Some(conv_ids), Some(titles), Some(created_ats), Some(updated_ats)) =
                (conv_ids, titles, created_ats, updated_ats)
            else {
                continue;
            };

            for i in 0..conv_ids.len() {
                let conv_id = conv_ids.value(i).to_string();
                let row_updated_at = DateTime::parse_from_rfc3339(updated_ats.value(i))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default();

                let entry = state_map.entry(conv_id.clone()).or_insert_with(|| SidebarState {
                    title: titles.value(i).to_string(),
                    created_at: DateTime::parse_from_rfc3339(created_ats.value(i))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_default(),
                    updated_at: row_updated_at,
                });

                if row_updated_at > entry.updated_at {
                    entry.updated_at = row_updated_at;
                    entry.title = titles.value(i).to_string();
                }
            }
        }

        let mut convs: Vec<Conversation> = state_map
            .into_iter()
            .map(|(id, s)| Conversation {
                id,
                title: s.title,
                messages: Vec::new(),
                created_at: s.created_at,
                updated_at: s.updated_at,
                is_temporary: false,
            })
            .collect();

        convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(convs)
    }

    /// 加载所有对话摘要（按 updated_at 降序），供搜索页使用。
    ///
    /// 包含 message_count 和 last_assistant_snippet 等搜索展示所需的完整字段。
    /// 注意：LanceDB 表是消息行级存储（每条消息一行），不能使用行级 limit
    /// 来限制对话数量，否则消息多的对话会"吃掉" limit 配额，导致其他对话丢失。
    /// 因此本方法加载全部行，在内存中按 conversation_id 聚合后返回所有对话摘要。
    pub async fn load_conversation_summary(&self) -> anyhow::Result<Vec<ConversationSummary>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table.query().execute().await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(Vec::new());
        }

        /// 中间聚合状态：追踪每个对话的消息数和最新助手消息。
        struct SummaryState {
            title: String,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            message_count: usize,
            last_assistant_content: String,
            last_assistant_time: DateTime<Utc>,
        }

        let mut state_map: std::collections::HashMap<String, SummaryState> = std::collections::HashMap::new();

        for batch in &batches {
            let conv_ids = batch.column_by_name("conversation_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let titles = batch.column_by_name("title")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let created_ats = batch.column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let updated_ats = batch.column_by_name("updated_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let msg_ids = batch.column_by_name("message_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let roles = batch.column_by_name("role")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let contents = batch.column_by_name("content")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let timestamps = batch.column_by_name("timestamp")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let Some(conv_ids) = conv_ids else { continue };
            let Some(titles) = titles else { continue };
            let Some(created_ats) = created_ats else { continue };
            let Some(updated_ats) = updated_ats else { continue };

            for i in 0..conv_ids.len() {
                let conv_id = conv_ids.value(i).to_string();
                let row_updated_at = DateTime::parse_from_rfc3339(updated_ats.value(i))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default();

                let entry = state_map.entry(conv_id.clone()).or_insert_with(|| SummaryState {
                    title: titles.value(i).to_string(),
                    created_at: DateTime::parse_from_rfc3339(created_ats.value(i))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_default(),
                    updated_at: row_updated_at,
                    message_count: 0,
                    last_assistant_content: String::new(),
                    last_assistant_time: DateTime::default(),
                });

                if row_updated_at > entry.updated_at {
                    entry.updated_at = row_updated_at;
                    entry.title = titles.value(i).to_string();
                }

                // 跳过 __empty 占位行
                if let Some(msg_ids) = msg_ids {
                    if msg_ids.value(i).ends_with("__empty") {
                        continue;
                    }
                }

                entry.message_count += 1;

                // 追踪最新助手消息
                if let (Some(roles), Some(contents), Some(timestamps)) = (roles, contents, timestamps) {
                    if roles.value(i) == "Assistant" {
                        let ts = DateTime::parse_from_rfc3339(timestamps.value(i))
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_default();
                        if ts > entry.last_assistant_time {
                            entry.last_assistant_content = contents.value(i).to_string();
                            entry.last_assistant_time = ts;
                        }
                    }
                }
            }
        }

        let mut summaries: Vec<ConversationSummary> = state_map
            .into_iter()
            .map(|(id, s)| ConversationSummary {
                id,
                title: s.title,
                created_at: s.created_at,
                updated_at: s.updated_at,
                message_count: s.message_count,
                last_assistant_snippet: truncate_str(&s.last_assistant_content, ASSISTANT_SNIPPET_MAX_LEN),
            })
            .collect();

        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(summaries)
    }

    pub async fn add_message(&self, conv_id: &str, message: &Message) -> anyhow::Result<()> {
        let meta = self.load_conversation_meta_by_id(conv_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conv_id))?;
        self.insert_message_row(conv_id, &meta.title, meta.created_at, Utc::now(), message).await
    }

    pub async fn update_message_content(&self, conv_id: &str, msg_id: &str, new_content: &str) -> anyhow::Result<()> {
        let meta = self.load_conversation_meta_by_id(conv_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conv_id))?;

        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;
        let predicate = format!(
            "conversation_id = '{}' AND message_id = '{}'",
            escape_sql(conv_id),
            escape_sql(msg_id)
        );
        table.delete(&predicate).await?;

        let old_msg = self.load_message_by_id(conv_id, msg_id).await?;
        let new_msg = Message {
            id: msg_id.to_string(),
            role: old_msg.as_ref().map(|m| m.role.clone()).unwrap_or(MessageRole::Assistant),
            content: new_content.to_string(),
            reasoning_content: old_msg.as_ref().and_then(|m| m.reasoning_content.clone()),
            timestamp: old_msg.as_ref().map(|m| m.timestamp).unwrap_or_else(|| Utc::now()),
            status: old_msg.as_ref().map(|m| m.status.clone()).unwrap_or(MessageStatus::Sent),
        };

        self.insert_message_row(conv_id, &meta.title, meta.created_at, Utc::now(), &new_msg).await
    }

    pub async fn rename_conversation(&self, conv_id: &str, new_title: &str) -> anyhow::Result<()> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;
        let escaped_title = escape_sql(new_title);
        let now = Utc::now().to_rfc3339();
        table.update()
            .only_if(format!("conversation_id = '{}'", escape_sql(conv_id)))
            .column("title", format!("'{}'", escaped_title))
            .column("updated_at", format!("'{}'", now))
            .execute()
            .await?;
        Ok(())
    }

    pub async fn delete_conversation(&self, conv_id: &str) -> anyhow::Result<()> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;
        let predicate = format!("conversation_id = '{}'", escape_sql(conv_id));
        table.delete(&predicate).await?;
        Ok(())
    }

    pub async fn conversation_exists(&self, conv_id: &str) -> bool {
        self.load_conversation_meta_by_id(conv_id).await
            .ok()
            .flatten()
            .is_some()
    }

    pub async fn update_last_message(&self, conv_id: &str, content: &str, status: MessageStatus) -> anyhow::Result<()> {
        let meta = self.load_conversation_meta_by_id(conv_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conv_id))?;

        let last_msg = self.load_last_message(conv_id).await?;
        if let Some(mut msg) = last_msg {
            let table = self.table.as_ref()
                .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;
            let predicate = format!(
                "conversation_id = '{}' AND message_id = '{}'",
                escape_sql(conv_id),
                escape_sql(&msg.id)
            );
            table.delete(&predicate).await?;

            msg.content = content.to_string();
            msg.status = status;
            self.insert_message_row(conv_id, &meta.title, meta.created_at, Utc::now(), &msg).await?;
        }
        Ok(())
    }

    pub async fn remove_last_message(&self, conv_id: &str) -> anyhow::Result<()> {
        let last_msg = self.load_last_message(conv_id).await?;
        if let Some(msg) = last_msg {
            let table = self.table.as_ref()
                .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;
            let predicate = format!(
                "conversation_id = '{}' AND message_id = '{}'",
                escape_sql(conv_id),
                escape_sql(&msg.id)
            );
            table.delete(&predicate).await?;
        }
        Ok(())
    }

    pub async fn search_fulltext(&self, query: &str, limit: usize) -> anyhow::Result<Vec<crate::models::memory::SearchResult>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let escaped = query.replace('\'', "''").replace('%', "\\%").replace('_', "\\_");
        let stream = table
            .query()
            .only_if(format!("content LIKE '%{}%' ESCAPE '\\\\'", escaped))
            .limit(limit)
            .execute()
            .await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for batch in &batches {
            let conv_ids = batch.column_by_name("conversation_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let msg_ids = batch.column_by_name("message_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let titles = batch.column_by_name("title")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let contents = batch.column_by_name("content")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let roles = batch.column_by_name("role")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let timestamps = batch.column_by_name("timestamp")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let created_ats = batch.column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let (Some(conv_ids), Some(msg_ids), Some(titles), Some(contents), Some(roles), Some(timestamps)) =
                (conv_ids, msg_ids, titles, contents, roles, timestamps)
            else {
                continue;
            };

            for i in 0..conv_ids.len() {
                let msg_id = msg_ids.value(i).to_string();
                if msg_id.ends_with("__empty") {
                    continue;
                }
                let content = contents.value(i).to_string();
                let snippet = if content.len() > 200 {
                    let end = content.char_indices().take_while(|(i, _)| *i < 200).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0);
                    content[..end].to_string()
                } else {
                    content
                };
                let timestamp = DateTime::parse_from_rfc3339(timestamps.value(i))
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default();
                let created_at = created_ats
                    .and_then(|a| DateTime::parse_from_rfc3339(a.value(i)).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default();

                results.push(crate::models::memory::SearchResult {
                    conversation_id: conv_ids.value(i).to_string(),
                    message_id: msg_id,
                    title: titles.value(i).to_string(),
                    content_snippet: snippet,
                    role: roles.value(i).to_string(),
                    timestamp,
                    score: 1.0,
                    search_type: crate::models::memory::SearchType::FullText,
                    created_at,
                    message_count: 0,
                    last_assistant_snippet: String::new(),
                });
            }
        }

        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (idx, r) in results.iter().enumerate() {
            match seen.entry(r.conversation_id.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let prev_idx = *e.get();
                    if r.content_snippet.len() > results[prev_idx].content_snippet.len() {
                        e.insert(idx);
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(idx);
                }
            }
        }
        let keep: std::collections::HashSet<usize> = seen.into_values().collect();
        let mut results: Vec<_> = results.into_iter().enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, r)| r)
            .collect();

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // 批量补全 message_count 和 last_assistant_snippet
        self.enrich_search_results(&mut results).await;

        Ok(results)
    }

    pub async fn search_semantic(&self, query_vector: &[f32], limit: usize) -> anyhow::Result<Vec<crate::models::memory::SearchResult>> {
        let hits = self.vector_store.search_turns(query_vector, limit)
            .await
            .unwrap_or_default();

        if hits.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for hit in hits {
            if hit.score < 0.65 {
                continue;
            }

            let qa_text = if !hit.user_content.is_empty() {
                format!("{}：{}\n\n{}：{}",
                    t!("search.question"), hit.user_content,
                    t!("search.answer"), hit.content)
            } else {
                hit.content.clone()
            };
            let snippet = if qa_text.len() > 200 {
                let end = qa_text.char_indices()
                    .take_while(|(idx, _)| *idx < 200)
                    .last()
                    .map(|(idx, c)| idx + c.len_utf8())
                    .unwrap_or(0);
                qa_text[..end].to_string()
            } else {
                qa_text
            };

            let timestamp = DateTime::parse_from_rfc3339(&hit.timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_default();

            results.push(crate::models::memory::SearchResult {
                conversation_id: hit.conversation_id,
                message_id: hit.message_id,
                title: String::new(), // filled by enrich_search_results
                content_snippet: snippet,
                role: "assistant".to_string(),
                timestamp,
                score: hit.score,
                search_type: crate::models::memory::SearchType::Semantic,
                created_at: Utc::now(), // filled by enrich_search_results
                message_count: 0, // filled by enrich_search_results
                last_assistant_snippet: String::new(), // filled by enrich_search_results
            });
        }

        // 按 conversation_id 去重，保留最高分
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (idx, r) in results.iter().enumerate() {
            match seen.entry(r.conversation_id.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let prev_idx = *e.get();
                    if r.score > results[prev_idx].score {
                        e.insert(idx);
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(idx);
                }
            }
        }
        let keep: std::collections::HashSet<usize> = seen.into_values().collect();
        let mut results: Vec<_> = results.into_iter().enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, r)| r)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        // 批量补全 message_count 和 last_assistant_snippet
        self.enrich_search_results(&mut results).await;

        Ok(results)
    }

    /// 批量补全搜索结果的 message_count 和 last_assistant_snippet。
    async fn enrich_search_results(&self, results: &mut [crate::models::memory::SearchResult]) {
        // 收集需要查询的 conversation_id
        let conv_ids: std::collections::HashSet<&str> = results.iter()
            .map(|r| r.conversation_id.as_str())
            .collect();

        if conv_ids.is_empty() {
            return;
        }

        // 一次性加载所有相关对话的摘要信息
        let summaries = match self.load_conversation_summary().await {
            Ok(s) => s,
            Err(_) => return,
        };

        let summary_map: std::collections::HashMap<&str, &ConversationSummary> = summaries.iter()
            .map(|s| (s.id.as_str(), s))
            .collect();

        for result in results.iter_mut() {
            if let Some(summary) = summary_map.get(result.conversation_id.as_str()) {
                result.title = summary.title.clone();
                result.message_count = summary.message_count;
                result.last_assistant_snippet = summary.last_assistant_snippet.clone();
            }
        }
    }

    pub async fn load_conversation_list_paged(&self, page: usize, page_size: usize) -> anyhow::Result<(Vec<ConversationSummary>, usize)> {
        let all = self.load_conversation_summary().await?;
        let total = all.len();
        let start = (page.saturating_sub(1)) * page_size;
        let end = std::cmp::min(start + page_size, total);
        let page_items = if start < total {
            all[start..end].to_vec()
        } else {
            Vec::new()
        };
        Ok((page_items, total))
    }
}
