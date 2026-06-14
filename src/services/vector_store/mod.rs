pub mod lancedb_store;

use async_trait::async_trait;
use crate::models::memory::{SearchHit, TurnEntry};

#[async_trait]
pub trait VectorStore: Send + Sync {
    /// 添加一个轮次（可能包含多个分块向量）
    async fn add_turn(&self, entry: TurnEntry) -> anyhow::Result<()>;

    /// 按轮次检索，返回最相关的分块（带轮次元数据）
    async fn search_turns(&self, query_vector: &[f32], top_k: usize) -> anyhow::Result<Vec<SearchHit>>;

    /// 按 assistant 消息 ID 删除关联的轮次分块
    async fn delete_by_assistant_message(&self, msg_id: &str) -> anyhow::Result<()>;

    /// 返回 `self` 的 `Any` 引用，用于 downcast 到具体类型。
    fn as_any(&self) -> &dyn std::any::Any;
}
