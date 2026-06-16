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
use crate::services::vector_store::lancedb_store::LanceDbStore;
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

/// 需要迁移的列定义：(列名, SQL 默认值表达式)。
const REQUIRED_COLUMNS: [(&str, &str); 1] = [
    ("reasoning_content", "''"),
];

/// 收集已有列中缺失的必需列。
pub fn collect_missing_columns(existing_columns: &[String]) -> Vec<(String, String)> {
    REQUIRED_COLUMNS
        .iter()
        .filter(|(col_name, _)| !existing_columns.iter().any(|ec| ec == *col_name))
        .map(|(col_name, default)| (col_name.to_string(), default.to_string()))
        .collect()
}

/// 打印迁移开始日志。
fn log_migration_start(missing: &[(String, String)]) {
    for (col_name, _) in missing {
        eprintln!("[xechat] Migrating conversations table: adding column '{}'", col_name);
    }
}

static STORE: OnceCell<ConversationStore> = OnceCell::new();

pub fn init_store(store: ConversationStore) -> anyhow::Result<()> {
    STORE.set(store).map_err(|_| anyhow::anyhow!("Store already initialized"))
}

/// 更新全局 store 的 vector_store（嵌入提供商变更时调用）。
pub fn update_vector_store(vs: Option<Arc<dyn crate::services::vector_store::VectorStore>>) {
    if let Some(store) = STORE.get() {
        match store.vector_store.write() {
            Ok(mut guard) => {
                eprintln!("[xechat] update_vector_store: setting to {} (ptr={:?})",
                    vs.as_ref().map(|v| format!("Some({:p})", v.as_ref())).unwrap_or_else(|| "None".to_string()),
                    vs.as_ref().map(|v| v.as_ref() as *const _));
                *guard = vs;
            }
            Err(e) => {
                eprintln!("[xechat] update_vector_store FAILED: RwLock poisoned: {}", e);
                // Recover from poison and force update
                let mut guard = e.into_inner();
                *guard = vs;
            }
        }
    } else {
        eprintln!("[xechat] update_vector_store FAILED: STORE not initialized");
    }
}

pub fn get_store() -> Option<&'static ConversationStore> {
    STORE.get()
}

pub struct ConversationStore {
    db: lancedb::Connection,
    table: Option<lancedb::Table>,
    vector_store: std::sync::RwLock<Option<Arc<dyn crate::services::vector_store::VectorStore>>>,
}

// ── 摘要聚合辅助类型（模块级，避免被 cargo-crap 计入主函数 CC） ──

/// 从 RecordBatch 提取摘要所需的列引用。
struct SummaryColumns<'a> {
    conv_ids: &'a StringArray,
    titles: &'a StringArray,
    created_ats: &'a StringArray,
    updated_ats: &'a StringArray,
    msg_ids: Option<&'a StringArray>,
    roles: Option<&'a StringArray>,
    contents: Option<&'a StringArray>,
    timestamps: Option<&'a StringArray>,
}

fn extract_summary_columns(batch: &RecordBatch) -> Option<SummaryColumns<'_>> {
    let conv_ids = batch.column_by_name("conversation_id")
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
    let titles = batch.column_by_name("title")
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
    let created_ats = batch.column_by_name("created_at")
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
    let updated_ats = batch.column_by_name("updated_at")
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
    Some(SummaryColumns {
        msg_ids: batch.column_by_name("message_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>()),
        roles: batch.column_by_name("role")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>()),
        contents: batch.column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>()),
        timestamps: batch.column_by_name("timestamp")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>()),
        conv_ids,
        titles,
        created_ats,
        updated_ats,
    })
}

/// 摘要聚合过程中的中间状态。
struct SummaryState {
    title: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    message_count: usize,
    last_assistant_content: String,
    last_assistant_time: DateTime<Utc>,
}

fn track_assistant_message(
    entry: &mut SummaryState,
    cols: &SummaryColumns<'_>,
    i: usize,
) {
    let (Some(roles), Some(contents), Some(timestamps)) = (cols.roles, cols.contents, cols.timestamps) else {
        return;
    };
    if roles.value(i) != "Assistant" {
        return;
    }
    let ts = DateTime::parse_from_rfc3339(timestamps.value(i))
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_default();
    if ts > entry.last_assistant_time {
        entry.last_assistant_content = contents.value(i).to_string();
        entry.last_assistant_time = ts;
    }
}

fn update_summary_entry(
    entry: &mut SummaryState,
    cols: &SummaryColumns<'_>,
    i: usize,
) {
    let row_updated_at = DateTime::parse_from_rfc3339(cols.updated_ats.value(i))
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_default();

    if row_updated_at > entry.updated_at {
        entry.updated_at = row_updated_at;
        entry.title = cols.titles.value(i).to_string();
    }

    // 跳过 __empty 占位行
    if let Some(msg_ids) = cols.msg_ids {
        if LanceDbStore::should_skip_empty_message(msg_ids.value(i)) {
            return;
        }
    }

    entry.message_count += 1;
    track_assistant_message(entry, cols, i);
}

impl ConversationStore {
    pub async fn open(path: &str, vector_store: Option<Arc<dyn crate::services::vector_store::VectorStore>>) -> anyhow::Result<Self> {
        let db = lancedb::connect(path).execute().await?;
        let table = match db.open_table(TABLE_NAME).execute().await {
            Ok(t) => Some(t),
            Err(_) => None,
        };
        Ok(Self {
            db,
            table,
            vector_store: std::sync::RwLock::new(vector_store),
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

    /// 应用缺失列的迁移。
    async fn apply_migration(
        table: &lancedb::Table,
        missing: Vec<(String, String)>,
    ) -> anyhow::Result<()> {
        use lancedb::table::NewColumnTransform;
        log_migration_start(&missing);
        let count = missing.len();
        table.add_columns(NewColumnTransform::SqlExpressions(missing), None).await?;
        eprintln!("[xechat] Migration complete: {} columns added", count);
        Ok(())
    }

    /// 检查并执行 schema 迁移，确保表包含所有必需列。
    ///
    /// LanceDB 支持通过 `add_columns` 添加新列。
    /// 新增列的默认值用 SQL 空字符串表达式填充。
    async fn migrate_schema(&mut self) -> anyhow::Result<()> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let schema = table.schema().await?;
        let existing_columns: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();

        let missing = collect_missing_columns(&existing_columns);

        if !missing.is_empty() {
            Self::apply_migration(table, missing).await?;
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

    /// 执行带过滤条件的查询并收集结果批次。
    async fn execute_filtered_query(
        &self,
        filter: &str,
        order_by: Option<Vec<ColumnOrdering>>,
        limit: usize,
        offset: Option<usize>,
    ) -> anyhow::Result<Vec<RecordBatch>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let mut query = table
            .query()
            .only_if(filter)
            .limit(limit);

        if let Some(ordering) = order_by {
            query = query.order_by(Some(ordering));
        }
        if let Some(off) = offset {
            query = query.offset(off);
        }

        let stream = query.execute().await?;
        Ok(stream.try_collect().await?)
    }

    async fn load_last_message(&self, conv_id: &str) -> anyhow::Result<Option<Message>> {
        let filter = format!("conversation_id = '{}'", escape_sql(conv_id));
        let batches = self.execute_filtered_query(
            &filter,
            Some(vec![ColumnOrdering::desc_nulls_last("timestamp".to_string())]),
            1,
            None,
        ).await?;

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
        let filter = format!("conversation_id = '{}'", escape_sql(conv_id));
        let batches = self.execute_filtered_query(
            &filter,
            Some(vec![ColumnOrdering::asc_nulls_last("timestamp".to_string())]),
            limit,
            Some(offset),
        ).await?;

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

        Ok(Self::aggregate_sidebar_from_batches(&batches))
    }

    /// 从 RecordBatch 批次聚合 sidebar 对话列表（按 updated_at 降序）。
    pub fn aggregate_sidebar_from_batches(batches: &[RecordBatch]) -> Vec<Conversation> {
        struct SidebarState {
            title: String,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
        }

        let mut state_map: std::collections::HashMap<String, SidebarState> = std::collections::HashMap::new();

        for batch in batches {
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
        convs
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

        Ok(Self::aggregate_summary_from_batches(&batches))
    }

    /// 从 RecordBatch 批次聚合对话摘要（按 updated_at 降序）。
    pub fn aggregate_summary_from_batches(batches: &[RecordBatch]) -> Vec<ConversationSummary> {

        let mut state_map: std::collections::HashMap<String, SummaryState> = std::collections::HashMap::new();

        for batch in batches {
            let Some(cols) = extract_summary_columns(batch) else { continue };

            for i in 0..cols.conv_ids.len() {
                let conv_id = cols.conv_ids.value(i).to_string();
                let entry = state_map.entry(conv_id.clone()).or_insert_with(|| SummaryState {
                    title: cols.titles.value(i).to_string(),
                    created_at: DateTime::parse_from_rfc3339(cols.created_ats.value(i))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_default(),
                    updated_at: DateTime::parse_from_rfc3339(cols.updated_ats.value(i))
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_default(),
                    message_count: 0,
                    last_assistant_content: String::new(),
                    last_assistant_time: DateTime::default(),
                });
                update_summary_entry(entry, &cols, i);
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
                last_assistant_snippet: LanceDbStore::truncate_snippet(&s.last_assistant_content, ASSISTANT_SNIPPET_MAX_LEN),
            })
            .collect();

        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        summaries
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

    /// 构建更新后的消息（保留原消息的 id、role、reasoning_content、timestamp）。
    ///
    /// 若原消息不存在，返回 `None`。
    #[inline]
    pub fn build_updated_message(old_msg: Option<&Message>, content: &str, status: MessageStatus) -> Option<Message> {
        let old = old_msg?;
        Some(Message {
            id: old.id.clone(),
            role: old.role.clone(),
            content: content.to_string(),
            reasoning_content: old.reasoning_content.clone(),
            timestamp: old.timestamp,
            status,
        })
    }

    pub async fn update_last_message(&self, conv_id: &str, content: &str, status: MessageStatus) -> anyhow::Result<()> {
        let meta = self.load_conversation_meta_by_id(conv_id).await?
            .ok_or_else(|| anyhow::anyhow!("Conversation not found: {}", conv_id))?;

        if let Some(old_msg) = self.load_last_message(conv_id).await? {
            if let Some(new_msg) = Self::build_updated_message(Some(&old_msg), content, status) {
                self.replace_message(conv_id, &meta, &mut { new_msg }, content, status).await?;
            }
        }
        Ok(())
    }

    /// 替换消息：删除旧行并插入更新后的行。
    async fn replace_message(
        &self,
        conv_id: &str,
        meta: &crate::Conversation,
        msg: &mut Message,
        content: &str,
        status: MessageStatus,
    ) -> anyhow::Result<()> {
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
        self.insert_message_row(conv_id, &meta.title, meta.created_at, Utc::now(), msg).await
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

        let results = Self::build_fulltext_results_from_batches(&batches);
        let mut results = Self::dedup_results_by_conversation(results, |curr, prev| {
            curr.content_snippet.len() > prev.content_snippet.len()
        });

        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // 批量补全 message_count 和 last_assistant_snippet
        self.enrich_search_results(&mut results).await;

        Ok(results)
    }

    /// 从 RecordBatch 批次构建全文搜索结果列表。
    pub fn build_fulltext_results_from_batches(batches: &[RecordBatch]) -> Vec<crate::models::memory::SearchResult> {
        let mut results = Vec::new();
        for batch in batches {
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
        results
    }

    /// 按 conversation_id 去重搜索结果，保留每个对话的"最佳"结果。
    ///
    /// `is_better` 闭包决定当同一对话出现多条结果时保留哪条：
    /// - 全文搜索：保留 snippet 最长的
    /// - 语义搜索：保留 score 最高的
    pub fn dedup_results_by_conversation<F>(
        results: Vec<crate::models::memory::SearchResult>,
        is_better: F,
    ) -> Vec<crate::models::memory::SearchResult>
    where
        F: Fn(&crate::models::memory::SearchResult, &crate::models::memory::SearchResult) -> bool,
    {
        let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (idx, r) in results.iter().enumerate() {
            match seen.entry(r.conversation_id.clone()) {
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let prev_idx = *e.get();
                    if is_better(r, &results[prev_idx]) {
                        e.insert(idx);
                    }
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(idx);
                }
            }
        }
        let keep: std::collections::HashSet<usize> = seen.into_values().collect();
        results.into_iter().enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, r)| r)
            .collect()
    }

    /// 将搜索命中转换为问答展示文本。
    ///
    /// 若命中包含用户消息，则格式化为 "问题：…\n\n回答：…"；
    /// 否则直接返回助手内容。
    #[inline]
    pub fn format_qa_text(hit: &crate::models::memory::SearchHit) -> String {
        if !hit.user_content.is_empty() {
            format!("{}：{}\n\n{}：{}",
                t!("search.question"), hit.user_content,
                t!("search.answer"), hit.content)
        } else {
            hit.content.clone()
        }
    }

    /// 将文本截断到指定字符数，确保不在多字节字符中间断开。
    #[inline]
    pub fn truncate_snippet(text: &str, max_chars: usize) -> String {
        if text.len() <= max_chars {
            return text.to_string();
        }
        let end = text.char_indices()
            .take_while(|(idx, _)| *idx < max_chars)
            .last()
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0);
        text[..end].to_string()
    }

    /// 将 SearchHit 转换为 SearchResult（不含去重和排序）。
    #[inline]
    pub fn hit_to_search_result(hit: &crate::models::memory::SearchHit) -> Option<crate::models::memory::SearchResult> {
        if hit.score < 0.65 {
            return None;
        }

        let qa_text = Self::format_qa_text(hit);
        let snippet = LanceDbStore::truncate_snippet(&qa_text, 200);

        let timestamp = DateTime::parse_from_rfc3339(&hit.timestamp)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_default();

        Some(crate::models::memory::SearchResult {
            conversation_id: hit.conversation_id.clone(),
            message_id: hit.message_id.clone(),
            title: String::new(), // filled by enrich_search_results
            content_snippet: snippet,
            role: "assistant".to_string(),
            timestamp,
            score: hit.score,
            search_type: crate::models::memory::SearchType::Semantic,
            created_at: Utc::now(), // filled by enrich_search_results
            message_count: 0, // filled by enrich_search_results
            last_assistant_snippet: String::new(), // filled by enrich_search_results
        })
    }

    /// 按 conversation_id 去重搜索结果，保留最高分，并按分数降序排列。
    #[inline]
    pub fn dedup_and_sort_by_score(results: Vec<crate::models::memory::SearchResult>) -> Vec<crate::models::memory::SearchResult> {
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
        results
    }

    /// 用新 embedder 重新分块并嵌入原始轮次数据。
    ///
    /// 支持断点续传：已嵌入的 turn 会自动跳过。
    /// 返回 `(成功数, 跳过数)`，部分成功时可通过再次调用继续未完成的 turn。
    pub async fn reembed_turns(
        &self,
        raw_turns: Vec<crate::services::vector_store::lancedb_store::RawTurn>,
        embedder: &Arc<dyn crate::services::embedder::Embedder>,
        on_progress: &(dyn Fn(usize, usize) + Send + Sync),
        force_rebuild: bool,
    ) -> anyhow::Result<(usize, usize)> {
        let vs_guard = self.vector_store.read().unwrap_or_else(|e| e.into_inner());
        let vs = match vs_guard.as_ref() {
            Some(vs) => vs,
            None => return Err(anyhow::anyhow!("vector store not available")),
        };
        let lancedb = vs.as_any()
            .downcast_ref::<crate::services::vector_store::lancedb_store::LanceDbStore>()
            .ok_or_else(|| anyhow::anyhow!("vector store is not LanceDbStore"))?;
        lancedb.reembed_turns(raw_turns, embedder, on_progress, force_rebuild).await
    }

    /// 从 conversations 表中提取所有用户/助手消息对，生成 RawTurn 列表。
    ///
    /// 直接从 LanceDB RecordBatch 流式提取，避免通过 `batches_to_conversations` 构建
    /// 完整的 Conversation+Message 对象树，大幅减少内存分配和字符串克隆。
    pub async fn load_all_turns_from_conversations(&self) -> anyhow::Result<Vec<crate::services::vector_store::lancedb_store::RawTurn>> {
        let table = self.table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("table not initialized"))?;

        let stream = table.query().execute().await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        if batches.is_empty() {
            return Ok(Vec::new());
        }

        // 只提取需要的 5 列，不加载 reasoning_content / status 等无关字段
        Ok(Self::extract_turns_from_batches(&batches))
    }

    /// 从单个 RecordBatch 中提取消息所需的 5 列，若任一列缺失则返回 None。
    fn extract_message_columns(
        batch: &RecordBatch,
    ) -> Option<(&StringArray, &StringArray, &StringArray, &StringArray, &StringArray)> {
        let conv_ids = batch.column_by_name("conversation_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
        let msg_ids = batch.column_by_name("message_id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
        let roles = batch.column_by_name("role")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
        let contents = batch.column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
        let timestamps = batch.column_by_name("timestamp")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())?;
        Some((conv_ids, msg_ids, roles, contents, timestamps))
    }

    /// 将按会话分组的消息 HashMap 转换为 RawTurn 列表。
    fn build_raw_turns_from_pairs(
        conv_messages: &std::collections::HashMap<String, Vec<(String, String, String, String)>>,
    ) -> Vec<crate::services::vector_store::lancedb_store::RawTurn> {
        let mut turns = Vec::new();
        for (conv_id, msgs) in conv_messages {
            let pairs = crate::services::vector_store::lancedb_store::LanceDbStore::pair_user_assistant_messages(msgs);
            for pair in pairs {
                turns.push(crate::services::vector_store::lancedb_store::RawTurn {
                    id: format!("{}:{}", pair.user_msg_id, pair.assistant_msg_id),
                    conversation_id: conv_id.clone(),
                    user_message_id: pair.user_msg_id,
                    assistant_message_id: pair.assistant_msg_id,
                    turn_index: 0,
                    user_content: pair.user_content,
                    assistant_content: pair.assistant_content,
                    timestamp: pair.timestamp,
                });
            }
        }
        turns
    }

    /// 直接从 RecordBatch 批次中流式提取 turn 对。
    ///
    /// 按 conversation_id 分组后，在每组内按顺序配对 User→Assistant 消息，
    /// 避免构建中间 Conversation/Message 结构体。
    fn extract_turns_from_batches(batches: &[RecordBatch]) -> Vec<crate::services::vector_store::lancedb_store::RawTurn> {
        use std::collections::HashMap;

        // (conversation_id, [messages ordered by row])
        let mut conv_messages: HashMap<String, Vec<(String, String, String, String)>> = HashMap::new();

        for batch in batches {
            let Some((conv_ids, msg_ids, roles, contents, timestamps)) = Self::extract_message_columns(batch) else {
                continue;
            };

            for i in 0..conv_ids.len() {
                let msg_id = msg_ids.value(i);
                if LanceDbStore::should_skip_empty_message(msg_id) {
                    continue;
                }

                conv_messages.entry(conv_ids.value(i).to_string())
                    .or_default()
                    .push((
                        msg_id.to_string(),
                        roles.value(i).to_string(),
                        contents.value(i).to_string(),
                        timestamps.value(i).to_string(),
                    ));
            }
        }

        let turns = Self::build_raw_turns_from_pairs(&conv_messages);
        eprintln!("[xechat] extract_turns_from_batches: {} convs -> {} turns", conv_messages.len(), turns.len());
        turns
    }

    pub async fn search_semantic(&self, query_vector: &[f32], limit: usize) -> anyhow::Result<Vec<crate::models::memory::SearchResult>> {
        let vs_guard = self.vector_store.read().unwrap_or_else(|e| e.into_inner());
        let vs = match vs_guard.as_ref() {
            Some(vs) => vs,
            None => {
                eprintln!("[xechat:search] vector_store is None");
                return Ok(Vec::new());
            }
        };
        // 诊断：打印 vector_store 实例类型和指针
        eprintln!("[xechat:search] vector_store type={} ptr={:p}",
            std::any::type_name_of_val(vs.as_ref()), vs.as_ref() as *const _);
        let hits = vs.search_turns(query_vector, limit)
            .await
            .unwrap_or_default();

        eprintln!("[xechat:search] hits={} scores={:?}",
            hits.len(),
            hits.iter().map(|h| h.score).take(5).collect::<Vec<_>>()
        );

        if hits.is_empty() {
            return Ok(Vec::new());
        }

        let results: Vec<_> = hits.iter()
            .filter_map(|hit| Self::hit_to_search_result(hit))
            .collect();

        eprintln!("[xechat:search] after filter (score>=0.65): {} results", results.len());
        if !hits.is_empty() && results.is_empty() {
            eprintln!("[xechat:search] WARNING: all hits filtered by score threshold. \
                Top score={:.4} < 0.65. This usually means stored/query vectors are in different spaces. \
                Try rebuilding vectors in Settings.", hits.iter().map(|h| h.score).fold(0.0f32, f32::max));
        }

        let mut results = Self::dedup_and_sort_by_score(results);

        // 批量补全 message_count 和 last_assistant_snippet
        self.enrich_search_results(&mut results).await;

        Ok(results)
    }

    /// 批量补全搜索结果的 message_count 和 last_assistant_snippet。
    async fn enrich_search_results(&self, results: &mut [crate::models::memory::SearchResult]) {
        let conv_ids: std::collections::HashSet<&str> = results.iter()
            .map(|r| r.conversation_id.as_str())
            .collect();

        if conv_ids.is_empty() {
            return;
        }

        let summary_map = self.load_summary_map().await;

        for result in results.iter_mut() {
            if let Some(summary) = summary_map.get(result.conversation_id.as_str()) {
                result.title = summary.title.clone();
                result.message_count = summary.message_count;
                result.last_assistant_snippet = summary.last_assistant_snippet.clone();
            }
        }
    }

    /// 加载对话摘要并构建以 id 为键的 HashMap。
    ///
    /// 加载失败时返回空 HashMap，调用方跳过 enrich 即可。
    async fn load_summary_map(&self) -> std::collections::HashMap<String, ConversationSummary> {
        match self.load_conversation_summary().await {
            Ok(summaries) => summaries.into_iter().map(|s| (s.id.clone(), s)).collect(),
            Err(_) => std::collections::HashMap::new(),
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
