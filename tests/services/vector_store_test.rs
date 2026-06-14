use async_trait::async_trait;
use xechat::models::memory::{SearchHit, TurnEntry};
use xechat::services::vector_store::VectorStore;

struct DummyStore;

#[async_trait]
impl VectorStore for DummyStore {
    async fn add_turn(&self, _entry: TurnEntry) -> anyhow::Result<()> {
        Ok(())
    }
    async fn search_turns(
        &self,
        _query_vector: &[f32],
        _top_k: usize,
    ) -> anyhow::Result<Vec<SearchHit>> {
        Ok(vec![])
    }
    async fn delete_by_assistant_message(&self, _msg_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn test_vector_store_trait_interface() {
    let _d = DummyStore;
}
