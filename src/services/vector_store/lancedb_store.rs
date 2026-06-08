use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use arrow_array::{
    Array, FixedSizeListArray, Int32Array, RecordBatch, StringArray, Float32Array,
};
use arrow_schema::{DataType, Field, Schema};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::index::Index;
use lancedb::index::vector::IvfPqIndexBuilder;
use lancedb::DistanceType;

use super::VectorStore;
use crate::models::memory::{SearchHit, TurnEntry};

const TURNS_TABLE_NAME: &str = "turns";
const VECTOR_DIM: i32 = 768;

/// 向量索引创建阈值：数据量 >= 此值时创建 IVF_PQ 索引
const VECTOR_INDEX_MIN_ROWS: usize = 1000;
/// 向量索引重建：数据增长 >= 此百分比时触发
const VECTOR_INDEX_REBUILD_GROWTH_PCT: u64 = 10;
/// 向量索引重建最小间隔（秒）
const VECTOR_INDEX_REBUILD_MIN_SECS: u64 = 6 * 3600;
/// 向量索引强制重建最大间隔（秒）
const VECTOR_INDEX_REBUILD_MAX_SECS: u64 = 24 * 3600;

pub struct LanceDbStore {
    db: lancedb::Connection,
    turns_table: Option<lancedb::Table>,
    /// 向量索引是否已创建
    vector_index_built: Arc<AtomicBool>,
    /// 上次索引构建时的行数
    vector_index_rows: Arc<AtomicUsize>,
    /// 上次索引构建时间（Unix 秒）
    vector_index_time: Arc<AtomicU64>,
}

impl LanceDbStore {
    pub async fn open(path: &str) -> anyhow::Result<Self> {
        let db = lancedb::connect(path).execute().await?;
        let turns_table = match db.open_table(TURNS_TABLE_NAME).execute().await {
            Ok(t) => Some(t),
            Err(_) => None,
        };
        Ok(Self {
            db,
            turns_table,
            vector_index_built: Arc::new(AtomicBool::new(false)),
            vector_index_rows: Arc::new(AtomicUsize::new(0)),
            vector_index_time: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn ensure_table(&mut self) -> anyhow::Result<()> {
        if self.turns_table.is_none() {
            let schema = Self::turns_arrow_schema();
            let batch = RecordBatch::new_empty(schema);
            let table = self
                .db
                .create_table(TURNS_TABLE_NAME, vec![batch])
                .execute()
                .await?;

            // 创建全文搜索倒排索引（空表也可创建）
            if let Err(e) = table
                .create_index(&["chunk_text"], Index::FTS(Default::default()))
                .execute()
                .await
            {
                eprintln!("[xechat] Failed to create FTS index on turns.chunk_text: {}", e);
            }

            // 创建标量索引，加速按 conversation_id / assistant_message_id 过滤和删除
            if let Err(e) = table
                .create_index(&["conversation_id"], Index::BTree(Default::default()))
                .execute()
                .await
            {
                eprintln!("[xechat] Failed to create BTree index on turns.conversation_id: {}", e);
            }
            if let Err(e) = table
                .create_index(&["assistant_message_id"], Index::BTree(Default::default()))
                .execute()
                .await
            {
                eprintln!("[xechat] Failed to create BTree index on turns.assistant_message_id: {}", e);
            }

            self.turns_table = Some(table);
        }

        Ok(())
    }

    /// 根据数据量与时间策略，决定是否创建或重建向量索引
    async fn maybe_rebuild_vector_index(&self) -> anyhow::Result<()> {
        let table = match self.turns_table.as_ref() {
            Some(t) => t,
            None => return Ok(()),
        };

        let count = table.count_rows(None).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let index_built = self.vector_index_built.load(Ordering::Relaxed);
        let last_rows = self.vector_index_rows.load(Ordering::Relaxed);
        let last_time = self.vector_index_time.load(Ordering::Relaxed);

        // 条件1：索引未创建且数据量达到阈值
        if !index_built && count >= VECTOR_INDEX_MIN_ROWS {
            self.build_vector_index(table).await?;
            self.vector_index_built.store(true, Ordering::Relaxed);
            self.vector_index_rows.store(count, Ordering::Relaxed);
            self.vector_index_time.store(now, Ordering::Relaxed);
            return Ok(());
        }

        // 条件2：索引已创建，检查是否需要重建
        if index_built {
            let growth_pct = if last_rows > 0 {
                ((count as u64 - last_rows as u64) * 100) / last_rows as u64
            } else {
                100
            };
            let elapsed = now.saturating_sub(last_time);

            let should_rebuild = growth_pct >= VECTOR_INDEX_REBUILD_GROWTH_PCT
                && elapsed >= VECTOR_INDEX_REBUILD_MIN_SECS;
            let force_rebuild = elapsed >= VECTOR_INDEX_REBUILD_MAX_SECS;

            if should_rebuild || force_rebuild {
                self.build_vector_index(table).await?;
                self.vector_index_rows.store(count, Ordering::Relaxed);
                self.vector_index_time.store(now, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    async fn build_vector_index(&self, table: &lancedb::Table) -> anyhow::Result<()> {
        table
            .create_index(
                &["vector"],
                Index::IvfPq(IvfPqIndexBuilder::default()
                    .distance_type(DistanceType::Cosine)),
            )
            .execute()
            .await?;
        Ok(())
    }

    fn turns_arrow_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("conversation_id", DataType::Utf8, false),
            Field::new("user_message_id", DataType::Utf8, false),
            Field::new("assistant_message_id", DataType::Utf8, false),
            Field::new("turn_index", DataType::Int32, false),
            Field::new("user_content", DataType::Utf8, false),
            Field::new("assistant_content", DataType::Utf8, false),
            Field::new("chunk_index", DataType::Int32, false),
            Field::new("chunk_text", DataType::Utf8, false),
            Field::new("start_char", DataType::Int32, false),
            Field::new("end_char", DataType::Int32, false),
            Field::new("timestamp", DataType::Utf8, false),
            Field::new(
                "vector",
                DataType::FixedSizeList(
                    Arc::new(Field::new("item", DataType::Float32, true)),
                    VECTOR_DIM,
                ),
                true,
            ),
        ]))
    }
}

#[async_trait]
impl VectorStore for LanceDbStore {
    async fn add_turn(&self, entry: TurnEntry) -> anyhow::Result<()> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;

        let n = entry.chunks.len();
        let ids = StringArray::from_iter_values(entry.chunks.iter().map(|_| entry.id.as_str()));
        let conv_ids = StringArray::from_iter_values(entry.chunks.iter().map(|_| entry.conversation_id.as_str()));
        let user_msg_ids = StringArray::from_iter_values(entry.chunks.iter().map(|_| entry.user_message_id.as_str()));
        let asst_msg_ids = StringArray::from_iter_values(entry.chunks.iter().map(|_| entry.assistant_message_id.as_str()));
        let turn_indices = Int32Array::from_iter_values(std::iter::repeat(entry.turn_index as i32).take(n));
        let user_contents = StringArray::from_iter_values(entry.chunks.iter().map(|_| entry.user_content.as_str()));
        let asst_contents = StringArray::from_iter_values(entry.chunks.iter().map(|_| entry.assistant_content.as_str()));
        let chunk_indices = Int32Array::from_iter_values(entry.chunks.iter().map(|c| c.chunk_index as i32));
        let chunk_texts = StringArray::from_iter_values(entry.chunks.iter().map(|c| c.chunk_text.as_str()));
        let start_chars = Int32Array::from_iter_values(entry.chunks.iter().map(|c| c.start_char as i32));
        let end_chars = Int32Array::from_iter_values(entry.chunks.iter().map(|c| c.end_char as i32));
        let timestamps = StringArray::from_iter_values(std::iter::repeat(entry.timestamp.to_rfc3339()).take(n));
        let vectors = FixedSizeListArray::from_iter_primitive::<arrow_array::types::Float32Type, _, _>(
            entry.chunks.iter().map(|c| {
                if c.embedding.len() == VECTOR_DIM as usize {
                    Some(c.embedding.iter().map(|v| Some(*v)).collect::<Vec<_>>())
                } else {
                    None
                }
            }),
            VECTOR_DIM,
        );

        let batch = RecordBatch::try_new(Self::turns_arrow_schema(), vec![
            Arc::new(ids), Arc::new(conv_ids), Arc::new(user_msg_ids),
            Arc::new(asst_msg_ids), Arc::new(turn_indices), Arc::new(user_contents),
            Arc::new(asst_contents), Arc::new(chunk_indices), Arc::new(chunk_texts),
            Arc::new(start_chars), Arc::new(end_chars), Arc::new(timestamps),
            Arc::new(vectors),
        ])?;

        table.add(vec![batch]).execute().await?;
        self.maybe_rebuild_vector_index().await?;
        Ok(())
    }

    async fn search_turns(&self, query_vector: &[f32], top_k: usize) -> anyhow::Result<Vec<SearchHit>> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;

        let stream = table
            .vector_search(query_vector)?
            .limit(top_k)
            .execute()
            .await?;
        let results: Vec<RecordBatch> = stream.try_collect().await?;

        let mut hits = Vec::new();
        for batch in results {
            let conv_ids = batch.column_by_name("conversation_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let user_msg_ids = batch.column_by_name("user_message_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let asst_msg_ids = batch.column_by_name("assistant_message_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let user_contents = batch.column_by_name("user_content").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let asst_contents = batch.column_by_name("assistant_content").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let chunk_indices = batch.column_by_name("chunk_index").and_then(|c| c.as_any().downcast_ref::<Int32Array>());
            let timestamps = batch.column_by_name("timestamp").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let dists = batch.column_by_name("_distance").and_then(|c| c.as_any().downcast_ref::<Float32Array>());

            if let (Some(dists), Some(asst_contents)) = (dists, asst_contents) {
                for i in 0..dists.len() {
                    hits.push(SearchHit {
                        score: 1.0 - dists.value(i),
                        entry_id: String::new(),
                        conversation_id: conv_ids.map(|c| c.value(i).to_string()).unwrap_or_default(),
                        message_id: asst_msg_ids.map(|c| c.value(i).to_string()).unwrap_or_default(),
                        content: asst_contents.value(i).to_string(),
                        role: "assistant".to_string(),
                        timestamp: timestamps.map(|c| c.value(i).to_string()).unwrap_or_default(),
                        user_message_id: user_msg_ids.map(|c| c.value(i).to_string()).unwrap_or_default(),
                        user_content: user_contents.map(|c| c.value(i).to_string()).unwrap_or_default(),
                        chunk_index: chunk_indices.map(|c| c.value(i)).unwrap_or(-1),
                    });
                }
            }
        }
        Ok(hits)
    }

    async fn delete_by_assistant_message(&self, msg_id: &str) -> anyhow::Result<()> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;
        let predicate = format!("assistant_message_id = '{}'", msg_id);
        table.delete(&predicate).await?;
        Ok(())
    }
}
