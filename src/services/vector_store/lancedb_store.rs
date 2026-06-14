use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::path::Path;

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
use serde::{Deserialize, Serialize};

use super::VectorStore;
use crate::models::memory::{SearchHit, TurnEntry};

const TURNS_TABLE_NAME: &str = "turns";
/// Qwen3-Embedding 的默认向量维度，用作未检测到已有表时的 fallback。
const DEFAULT_VECTOR_DIM: i32 = 1024;
/// embedder 元数据文件名，存储在 LanceDB 目录下。
const EMBEDDER_META_FILE: &str = ".embedder_meta.json";

/// 向量索引创建阈值：数据量 >= 此值时创建 IVF_PQ 索引
const VECTOR_INDEX_MIN_ROWS: usize = 10000;
/// 向量索引重建：数据增长 >= 此百分比时触发
const VECTOR_INDEX_REBUILD_GROWTH_PCT: u64 = 10;
/// 向量索引重建最小间隔（秒）
const VECTOR_INDEX_REBUILD_MIN_SECS: u64 = 6 * 3600;
/// 向量索引强制重建最大间隔（秒）
const VECTOR_INDEX_REBUILD_MAX_SECS: u64 = 24 * 3600;

/// embedder 元数据，记录创建 turns 表时使用的嵌入器信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbedderMeta {
    /// 嵌入器名称标识（如 "qwen3-embedding-0.6b"、"ollama"）
    pub name: String,
    /// 向量维度
    pub dimension: i32,
}

/// 从 turns 表读取的原始轮次数据，用于切换 embedder 时重新嵌入。
///
/// 只保留文本字段，不包含向量（向量需要用新 embedder 重新生成）。
#[derive(Debug, Clone)]
pub struct RawTurn {
    pub id: String,
    pub conversation_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
    pub turn_index: i32,
    pub user_content: String,
    pub assistant_content: String,
    pub timestamp: String,
}

pub struct LanceDbStore {
    db: lancedb::Connection,
    /// LanceDB 目录路径，用于存储 embedder 元数据文件。
    lancedb_path: String,
    turns_table: Option<lancedb::Table>,
    /// 向量维度（从 embedder 或已有表 schema 检测）
    vector_dim: i32,
    /// 向量索引是否已创建
    vector_index_built: Arc<AtomicBool>,
    /// 上次索引构建时的行数
    vector_index_rows: Arc<AtomicUsize>,
    /// 上次索引构建时间（Unix 秒）
    vector_index_time: Arc<AtomicU64>,
}

impl LanceDbStore {
    pub async fn open(path: &str) -> anyhow::Result<Self> {
        let db = lancedb::connect(path)
            .read_consistency_interval(std::time::Duration::ZERO)
            .execute().await?;
        let (turns_table, vector_dim) = match db.open_table(TURNS_TABLE_NAME).execute().await {
            Ok(t) => {
                let dim = Self::detect_vector_dim_from_table(&t).await;
                (Some(t), dim)
            }
            Err(_) => (None, Self::resolve_vector_dim()),
        };
        Ok(Self {
            db,
            lancedb_path: path.to_string(),
            turns_table,
            vector_dim,
            vector_index_built: Arc::new(AtomicBool::new(false)),
            vector_index_rows: Arc::new(AtomicUsize::new(0)),
            vector_index_time: Arc::new(AtomicU64::new(0)),
        })
    }

    /// 从全局 embedder 获取向量维度，fallback 到默认值。
    fn resolve_vector_dim() -> i32 {
        crate::services::embedder::get_embedder()
            .map(|e| e.dimension() as i32)
            .unwrap_or(DEFAULT_VECTOR_DIM)
    }

    /// 从已有表的 schema 中检测向量列维度。
    async fn detect_vector_dim_from_table(table: &lancedb::Table) -> i32 {
        match table.schema().await {
            Ok(schema) => {
                for field in schema.fields() {
                    if field.name() == "vector" {
                        if let arrow_schema::DataType::FixedSizeList(_, dim) = field.data_type() {
                            return *dim;
                        }
                    }
                }
                Self::resolve_vector_dim()
            }
            Err(_) => Self::resolve_vector_dim(),
        }
    }

    pub async fn ensure_table(&mut self) -> anyhow::Result<()> {
        if self.turns_table.is_none() {
            let schema = Self::turns_arrow_schema(self.vector_dim);
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
            // id 列索引：add_turn upsert 时按 id 删除旧行需要
            if let Err(e) = table
                .create_index(&["id"], Index::BTree(Default::default()))
                .execute()
                .await
            {
                eprintln!("[xechat] Failed to create BTree index on turns.id: {}", e);
            }

            self.turns_table = Some(table);
        }

        Ok(())
    }

    /// 删除旧 turns 表并重置状态，以便 `ensure_table` 用新维度重建。
    pub async fn drop_and_recreate_turns_table(&mut self) {
        if let Err(e) = self.db.drop_table(TURNS_TABLE_NAME, &[]).await {
            eprintln!("[xechat] Failed to drop turns table: {}", e);
        }
        self.turns_table = None;
        self.vector_index_built.store(false, Ordering::Relaxed);
        self.vector_index_rows.store(0, Ordering::Relaxed);
        self.vector_index_time.store(0, Ordering::Relaxed);
    }

    /// 获取当前向量维度。
    pub fn vector_dim(&self) -> i32 {
        self.vector_dim
    }

    /// 设置向量维度（维度不匹配重建后调用）。
    pub fn set_vector_dim(&mut self, dim: i32) {
        self.vector_dim = dim;
    }

    /// 读取 embedder 元数据文件。
    ///
    /// 返回 `None` 表示文件不存在（首次运行或旧版本）。
    pub fn read_embedder_meta(&self) -> Option<EmbedderMeta> {
        let path = Path::new(&self.lancedb_path).join(EMBEDDER_META_FILE);
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// 写入 embedder 元数据文件。
    pub fn write_embedder_meta(&self, meta: &EmbedderMeta) {
        let path = Path::new(&self.lancedb_path).join(EMBEDDER_META_FILE);
        if let Ok(content) = serde_json::to_string_pretty(meta) {
            if let Err(e) = std::fs::write(&path, content) {
                eprintln!("[xechat] Failed to write embedder meta: {}", e);
            }
        }
    }

    /// 检测当前 embedder 与已记录的元数据是否匹配。
    ///
    /// 返回 `Some(current_meta)` 表示不匹配（需要重建），`None` 表示匹配或无历史记录。
    pub fn check_embedder_changed(&self) -> Option<EmbedderMeta> {
        let embedder = crate::services::embedder::get_embedder()?;
        let current = EmbedderMeta {
            name: embedder.name().to_string(),
            dimension: embedder.dimension() as i32,
        };
        match self.read_embedder_meta() {
            Some(saved) if saved == current => None,
            Some(saved) => {
                eprintln!(
                    "[xechat] Embedder changed: saved={:?} current={:?}",
                    saved, current
                );
                Some(current)
            }
            None => {
                // 首次运行或旧版本无元数据文件
                // 如果 turns 表已有数据，需要重建（可能编码方式不同）
                // 如果 turns 表为空，直接写入元数据即可
                let has_data = self.turns_table.is_some();
                if has_data {
                    eprintln!(
                        "[xechat] No embedder meta but turns table has data, triggering rebuild"
                    );
                    Some(current)
                } else {
                    self.write_embedder_meta(&current);
                    None
                }
            }
        }
    }

    /// 判断是否需要首次创建向量索引
    pub fn needs_initial_index(index_built: bool, count: usize) -> bool {
        !index_built && count >= VECTOR_INDEX_MIN_ROWS
    }

    /// 判断是否需要重建向量索引（基于增长率或最大间隔）
    pub fn needs_rebuild(index_built: bool, count: usize, last_rows: usize, last_time: u64, now: u64) -> bool {
        if !index_built {
            return false;
        }
        let growth_pct = if last_rows > 0 {
            ((count as u64 - last_rows as u64) * 100) / last_rows as u64
        } else {
            100
        };
        let elapsed = now.saturating_sub(last_time);
        let should_rebuild = growth_pct >= VECTOR_INDEX_REBUILD_GROWTH_PCT
            && elapsed >= VECTOR_INDEX_REBUILD_MIN_SECS;
        let force_rebuild = elapsed >= VECTOR_INDEX_REBUILD_MAX_SECS;
        should_rebuild || force_rebuild
    }

    /// 更新索引构建后的统计信息
    fn update_index_stats(&self, count: usize, now: u64) {
        self.vector_index_built.store(true, Ordering::Relaxed);
        self.vector_index_rows.store(count, Ordering::Relaxed);
        self.vector_index_time.store(now, Ordering::Relaxed);
    }

    /// 获取当前 Unix 时间戳（秒）。
    pub fn current_timestamp_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// 根据条件重建向量索引（首次创建或增量重建）。
    pub async fn rebuild_index_if_needed(
        &self,
        table: &lancedb::Table,
        count: usize,
        last_rows: usize,
        last_time: u64,
        now: u64,
    ) -> anyhow::Result<()> {
        let index_built = self.vector_index_built.load(Ordering::Relaxed);

        if Self::needs_initial_index(index_built, count) {
            self.build_vector_index(table).await?;
            self.update_index_stats(count, now);
            return Ok(());
        }

        if Self::needs_rebuild(index_built, count, last_rows, last_time, now) {
            self.build_vector_index(table).await?;
            self.vector_index_rows.store(count, Ordering::Relaxed);
            self.vector_index_time.store(now, Ordering::Relaxed);
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
        let now = Self::current_timestamp_secs();

        let last_rows = self.vector_index_rows.load(Ordering::Relaxed);
        let last_time = self.vector_index_time.load(Ordering::Relaxed);

        self.rebuild_index_if_needed(table, count, last_rows, last_time, now).await
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

    /// 将 RecordBatch 转换为 SearchHit 列表
    pub fn batch_to_hits(batch: &RecordBatch) -> Vec<SearchHit> {
        let conv_ids = batch.column_by_name("conversation_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let user_msg_ids = batch.column_by_name("user_message_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let asst_msg_ids = batch.column_by_name("assistant_message_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let user_contents = batch.column_by_name("user_content").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let asst_contents = batch.column_by_name("assistant_content").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let chunk_indices = batch.column_by_name("chunk_index").and_then(|c| c.as_any().downcast_ref::<Int32Array>());
        let timestamps = batch.column_by_name("timestamp").and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let dists = batch.column_by_name("_distance").and_then(|c| c.as_any().downcast_ref::<Float32Array>());

        let (Some(dists), Some(asst_contents)) = (dists, asst_contents) else {
            return Vec::new();
        };

        (0..dists.len())
            .map(|i| SearchHit {
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
            })
            .collect()
    }

    fn turns_arrow_schema(vector_dim: i32) -> Arc<Schema> {
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
                    vector_dim,
                ),
                true,
            ),
        ]))
    }

    /// 构建 TurnEntry 的向量列（FixedSizeListArray）。
    fn build_vectors_array(entry: &crate::models::memory::TurnEntry, vector_dim: i32) -> FixedSizeListArray {
        FixedSizeListArray::from_iter_primitive::<arrow_array::types::Float32Type, _, _>(
            entry.chunks.iter().map(|c| {
                if c.embedding.len() == vector_dim as usize {
                    Some(c.embedding.iter().map(|v| Some(*v)).collect::<Vec<_>>())
                } else {
                    None
                }
            }),
            vector_dim,
        )
    }

    /// 构建 TurnEntry 的 RecordBatch。
    fn build_turn_batch(entry: &crate::models::memory::TurnEntry, vector_dim: i32) -> anyhow::Result<RecordBatch> {
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
        let vectors = Self::build_vectors_array(entry, vector_dim);

        Ok(RecordBatch::try_new(Self::turns_arrow_schema(vector_dim), vec![
            Arc::new(ids), Arc::new(conv_ids), Arc::new(user_msg_ids),
            Arc::new(asst_msg_ids), Arc::new(turn_indices), Arc::new(user_contents),
            Arc::new(asst_contents), Arc::new(chunk_indices), Arc::new(chunk_texts),
            Arc::new(start_chars), Arc::new(end_chars), Arc::new(timestamps),
            Arc::new(vectors),
        ])?)
    }
}

#[async_trait]
impl VectorStore for LanceDbStore {
    async fn add_turn(&self, entry: TurnEntry) -> anyhow::Result<()> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;

        // Upsert 语义：先删除同 id 的旧行，再追加新行
        // 避免反复重建导致同一 turn 的行无限累积（row_count 膨胀）
        let predicate = format!("id = '{}'", entry.id.replace('\'', "\\'"));
        if let Err(e) = table.delete(&predicate).await {
            eprintln!("[xechat:add_turn] warning: failed to delete existing rows for upsert: {}", e);
        }

        let batch = Self::build_turn_batch(&entry, self.vector_dim)?;
        table.add(vec![batch]).execute().await?;
        self.maybe_rebuild_vector_index().await?;
        Ok(())
    }

    async fn search_turns(&self, query_vector: &[f32], top_k: usize) -> anyhow::Result<Vec<SearchHit>> {
        let results = self.execute_vector_search(query_vector, top_k).await?;
        let hits: Vec<SearchHit> = results.iter().flat_map(Self::batch_to_hits).collect();
        Ok(hits)
    }

    async fn delete_by_assistant_message(&self, msg_id: &str) -> anyhow::Result<()> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;
        let predicate = format!("assistant_message_id = '{}'", msg_id);
        table.delete(&predicate).await?;
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LanceDbStore {
    /// 执行向量搜索并收集结果批次。
    async fn execute_vector_search(&self, query_vector: &[f32], top_k: usize) -> anyhow::Result<Vec<RecordBatch>> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;

        // 诊断：搜索前计数行数 + 打印查询向量前5维
        match table.count_rows(None).await {
            Ok(count) => eprintln!("[xechat:search] LanceDB table row_count={} query_vec_first5={:?}",
                count, &query_vector[..5.min(query_vector.len())]),
            Err(e) => eprintln!("[xechat:search] Failed to count rows: {}", e),
        }

        // 诊断：读取表中实际存储的向量，与写入时对比
        {
            let scan_stream = table.query().execute().await;
            if let Ok(stream) = scan_stream {
                let scan_batches: Vec<RecordBatch> = stream.try_collect().await.unwrap_or_default();
                for (bi, batch) in scan_batches.iter().enumerate() {
                    let ids = batch.column_by_name("id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
                    let vectors = batch.column_by_name("vector").and_then(|c| c.as_any().downcast_ref::<FixedSizeListArray>());
                    if let (Some(ids), Some(vectors)) = (ids, vectors) {
                        for ri in 0..ids.len().min(4) {
                            let id = ids.value(ri);
                            if let Some(vec_val) = vectors.value(ri).as_any().downcast_ref::<Float32Array>() {
                                let stored: Vec<f32> = vec_val.values().to_vec();
                                // 计算与查询向量的手动 cosine similarity
                                let dot: f32 = stored.iter().zip(query_vector.iter()).map(|(a, b)| a * b).sum();
                                let norm_s: f32 = stored.iter().map(|v| v * v).sum::<f32>().sqrt();
                                let norm_q: f32 = query_vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                                let manual_cosine = if norm_s > 0.0 && norm_q > 0.0 { dot / (norm_s * norm_q) } else { 0.0 };
                                eprintln!("[xechat:search] stored_vec[batch={},row={},id={}] first5={:?} manual_cosine_with_query={:.6}",
                                    bi, ri, id, &stored[..5.min(stored.len())], manual_cosine);
                            }
                        }
                    }
                }
            }
        }

        let stream = table
            .vector_search(query_vector)?
            .distance_type(DistanceType::Cosine)
            .limit(top_k)
            .execute()
            .await?;

        let batches: Vec<RecordBatch> = stream.try_collect().await?;
        Ok(batches)
    }

    /// 读取 turns 表中所有轮次的原始文本数据。
    ///
    /// 按 `id` 去重（同一轮次可能有多个分块行），只保留文本字段。
    /// 用于切换 embedder 时重新分块和嵌入。
    pub async fn read_all_turns_raw(&self) -> anyhow::Result<Vec<RawTurn>> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;

        let stream = table.query().execute().await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;

        let mut seen_ids = std::collections::HashSet::new();
        let mut turns = Vec::new();

        for batch in &batches {
            let ids = batch.column_by_name("id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let conv_ids = batch.column_by_name("conversation_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let user_msg_ids = batch.column_by_name("user_message_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let asst_msg_ids = batch.column_by_name("assistant_message_id").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let turn_indices = batch.column_by_name("turn_index").and_then(|c| c.as_any().downcast_ref::<Int32Array>());
            let user_contents = batch.column_by_name("user_content").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let asst_contents = batch.column_by_name("assistant_content").and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let timestamps = batch.column_by_name("timestamp").and_then(|c| c.as_any().downcast_ref::<StringArray>());

            let (Some(ids), Some(conv_ids), Some(user_contents), Some(asst_contents)) =
                (ids, conv_ids, user_contents, asst_contents)
            else {
                continue;
            };

            for i in 0..ids.len() {
                let id = ids.value(i).to_string();
                if seen_ids.insert(id.clone()) {
                    turns.push(RawTurn {
                        id,
                        conversation_id: conv_ids.value(i).to_string(),
                        user_message_id: user_msg_ids.map(|c| c.value(i).to_string()).unwrap_or_default(),
                        assistant_message_id: asst_msg_ids.map(|c| c.value(i).to_string()).unwrap_or_default(),
                        turn_index: turn_indices.map(|c| c.value(i)).unwrap_or(0),
                        user_content: user_contents.value(i).to_string(),
                        assistant_content: asst_contents.value(i).to_string(),
                        timestamp: timestamps.map(|c| c.value(i).to_string()).unwrap_or_default(),
                    });
                }
            }
        }

        Ok(turns)
    }

    /// 统计 turns 表的行数。
    pub async fn count_turns_rows(&self) -> anyhow::Result<usize> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;
        Ok(table.count_rows(None).await?)
    }

    /// 用新 embedder 重新分块并嵌入原始轮次数据，写回 turns 表。
    ///
    /// 调用前应已完成 `drop_and_recreate_turns_table` + `ensure_table`。
    /// 支持断点续传：每个 turn 嵌入后立即写入 LanceDB，失败时跳过继续下一个。
    ///
    /// 返回 `(成功数, 跳过/失败数)`，调用方可据此判断是否需要重试。
    pub async fn reembed_turns(
        &self,
        raw_turns: Vec<RawTurn>,
        embedder: &Arc<dyn crate::services::embedder::Embedder>,
        on_progress: &(dyn Fn(usize, usize) + Send + Sync),
    ) -> anyhow::Result<(usize, usize)> {
        if raw_turns.is_empty() {
            eprintln!("[xechat] reembed_turns: no turns to process");
            return Ok((0, 0));
        }

        eprintln!("[xechat] reembed_turns: starting {} turns with embedder {}", raw_turns.len(), embedder.name());
        eprintln!("[xechat] reembed_turns: turns_table={} vector_dim={}",
            self.turns_table.is_some(), self.vector_dim);
        // 打印输入 turn ID 样本，与 existing_ids 对比
        for t in raw_turns.iter().take(3) {
            eprintln!("[xechat]   input_turn sample: id={} conv_id={}", &t.id[..8.min(t.id.len())], &t.conversation_id[..8.min(t.conversation_id.len())]);
        }

        // 阶段 0：读取已存在的 turn ID，支持断点续传
        let existing_ids = self.get_existing_turn_ids().await.unwrap_or_default();
        if !existing_ids.is_empty() {
            eprintln!("[xechat] reembed_turns: skipping {} already-embedded turns", existing_ids.len());
        }

        let chunk_params = crate::services::embedder::ChunkParams::from_context_window(
            embedder.context_window()
        );

        let total = raw_turns.len();
        let mut success_count = 0;
        let mut skipped_count = 0;

        for (i, turn) in raw_turns.into_iter().enumerate() {
            // 断点续传：跳过已存在的 turn
            if existing_ids.contains(&turn.id) {
                eprintln!("[xechat:reembed] turn {}/{} SKIPPED (already exists): {}", i + 1, total, &turn.id[..8.min(turn.id.len())]);
                skipped_count += 1;
                on_progress(i + 1, total);
                continue;
            }

            let turn_text = format!("用户：{}\n助手：{}", turn.user_content, turn.assistant_content);
            let char_count = turn_text.chars().count();

            // 分块
            let (chunk_texts, chunk_indices, chunk_starts, chunk_ends) = if char_count < chunk_params.target_chars {
                let len = turn_text.len() as u32;
                (vec![turn_text], vec![0u32], vec![0u32], vec![len])
            } else {
                let spans = crate::services::embedder::manager::semantic_chunk(&turn_text, chunk_params);
                let mut texts = Vec::with_capacity(spans.len());
                let mut indices = Vec::with_capacity(spans.len());
                let mut starts = Vec::with_capacity(spans.len());
                let mut ends = Vec::with_capacity(spans.len());
                for (ci, span) in spans.iter().enumerate() {
                    texts.push(span.text.clone());
                    indices.push(ci as u32);
                    starts.push(span.start as u32);
                    ends.push(span.end as u32);
                }
                (texts, indices, starts, ends)
            };

            // 编码该 turn 的所有分块（单条失败自动重试，最多 3 次）
            const MAX_ENCODE_RETRIES: u32 = 3;
            const RETRY_DELAY_SECS: u64 = 2;
            let mut chunks = Vec::with_capacity(chunk_texts.len());
            let mut encode_ok = true;
            for (ci, text) in chunk_texts.iter().enumerate() {
                let mut last_err = None;
                for retry in 0..MAX_ENCODE_RETRIES {
                    if retry > 0 {
                        eprintln!("[xechat:reembed] turn {}/{} chunk {}/{} retry {}/{}", i + 1, total, ci + 1, chunk_texts.len(), retry, MAX_ENCODE_RETRIES);
                        tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                    }
                    match embedder.encode_passage(text).await {
                        Ok(embedding) => {
                            chunks.push(crate::models::memory::ChunkMeta {
                                chunk_index: chunk_indices[ci],
                                chunk_text: text.clone(),
                                start_char: chunk_starts[ci],
                                end_char: chunk_ends[ci],
                                embedding,
                            });
                            last_err = None;
                            break;
                        }
                        Err(e) => {
                            last_err = Some(e);
                        }
                    }
                }
                if let Some(e) = last_err {
                    eprintln!("[xechat:reembed] turn {}/{} chunk {}/{} encode FAILED after {} retries: {}", i + 1, total, ci + 1, chunk_texts.len(), MAX_ENCODE_RETRIES, e);
                    encode_ok = false;
                    break; // 该 turn 的任一分块失败则整个 turn 跳过
                }
            }

            if !encode_ok || chunks.is_empty() {
                eprintln!("[xechat:reembed] turn {}/{} SKIPPED due to encode error", i + 1, total);
                skipped_count += 1;
                on_progress(i + 1, total);
                continue;
            }

            // 写入 LanceDB（立即持久化）
            let timestamp = chrono::DateTime::parse_from_rfc3339(&turn.timestamp)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());

            let entry = crate::models::memory::TurnEntry {
                id: turn.id,
                conversation_id: turn.conversation_id,
                user_message_id: turn.user_message_id,
                assistant_message_id: turn.assistant_message_id,
                turn_index: turn.turn_index as u32,
                user_content: turn.user_content,
                assistant_content: turn.assistant_content,
                timestamp,
                chunks,
            };

            if let Err(e) = self.add_turn(entry).await {
                eprintln!("[xechat:reembed] turn {}/{} write FAILED: {}", i + 1, total, e);
                skipped_count += 1;
            } else {
                success_count += 1;
            }

            on_progress(i + 1, total);
        }

        eprintln!("[xechat] reembed_turns: done — success={}, skipped={}", success_count, skipped_count);

        if skipped_count > 0 && success_count == 0 {
            anyhow::bail!("All {} turns failed or were skipped", skipped_count);
        }

        Ok((success_count, skipped_count))
    }

    /// 读取 turns 表中已有的 turn ID 集合，用于断点续传时跳过已嵌入的轮次。
    async fn get_existing_turn_ids(&self) -> anyhow::Result<std::collections::HashSet<String>> {
        let table = self.turns_table.as_ref()
            .ok_or_else(|| anyhow::anyhow!("turns table not initialized"))?;

        // 诊断：打印表状态，确认断点续传基础数据是否正确
        let row_count = table.count_rows(None).await.unwrap_or(0);
        eprintln!("[xechat:reembed] get_existing_turn_ids: table row_count={}", row_count);

        let stream = table.query().execute().await?;
        let batches: Vec<RecordBatch> = stream.try_collect().await?;
        let mut ids = std::collections::HashSet::new();

        for batch in &batches {
            if let Some(id_col) = batch.column_by_name("id").and_then(|c| c.as_any().downcast_ref::<StringArray>()) {
                for i in 0..id_col.len() {
                    ids.insert(id_col.value(i).to_string());
                }
            }
        }

        // 打印唯一 ID 数量和样本，用于诊断断点续传是否生效
        eprintln!("[xechat:reembed] get_existing_turn_ids: unique_ids={}", ids.len());
        if !ids.is_empty() {
            let mut sample: Vec<_> = ids.iter().take(3).collect();
            sample.sort();
            for s in &sample {
                eprintln!("[xechat:reembed]   existing_id sample: {}", s);
            }
        }

        Ok(ids)
    }
}
