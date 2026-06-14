use std::sync::Arc;

use async_trait::async_trait;
use xechat::models::memory::{SearchHit, TurnEntry};
use xechat::services::embedder::Embedder;
use xechat::services::memory::MemoryPipeline;
use xechat::services::vector_store::VectorStore;

struct DummyEmbedder;

#[async_trait]
impl Embedder for DummyEmbedder {
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.1; 768]).collect())
    }
    async fn encode_one(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.1; 768])
    }
    fn dimension(&self) -> usize {
        768
    }
    fn name(&self) -> &str {
        "dummy"
    }
}

struct DummyVectorStore;

#[async_trait]
impl VectorStore for DummyVectorStore {
    async fn add_turn(&self, _entry: TurnEntry) -> anyhow::Result<()> {
        Ok(())
    }
    async fn search_turns(&self, _query_vector: &[f32], _top_k: usize) -> anyhow::Result<Vec<SearchHit>> {
        Ok(vec![])
    }
    async fn delete_by_assistant_message(&self, _msg_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::test]
async fn test_pipeline_preprocess_no_memory() {
    let pipeline = MemoryPipeline::new(
        Arc::new(DummyEmbedder),
        Arc::new(DummyVectorStore),
    );
    let result = pipeline.preprocess("什么是Rust？", &[]).await;
    assert!(!result.memory_used);
}

#[tokio::test]
async fn test_pipeline_preprocess_with_memory_trigger_but_no_hits() {
    let pipeline = MemoryPipeline::new(
        Arc::new(DummyEmbedder),
        Arc::new(DummyVectorStore),
    );
    let result = pipeline.preprocess("之前我们讨论过什么？", &[]).await;
    assert!(!result.memory_used);
}

#[tokio::test]
async fn test_pipeline_postprocess() {
    let pipeline = MemoryPipeline::new(
        Arc::new(DummyEmbedder),
        Arc::new(DummyVectorStore),
    );
    let result = pipeline.postprocess("conv-1", "msg-1", "test content").await;
    assert!(result.is_ok());
}

#[test]
fn test_init_pipeline_replaces_previous() {
    // 首次初始化
    let pipeline1 = MemoryPipeline::new(
        Arc::new(DummyEmbedder),
        Arc::new(DummyVectorStore),
    );
    assert!(xechat::services::memory::init_pipeline(pipeline1).is_ok());
    assert!(xechat::services::memory::get_pipeline().is_some());

    // 重复初始化不应报错（OnceCell 会报 "already initialized"，RwLock 允许覆盖）
    let pipeline2 = MemoryPipeline::new(
        Arc::new(DummyEmbedder),
        Arc::new(DummyVectorStore),
    );
    assert!(xechat::services::memory::init_pipeline(pipeline2).is_ok());
    assert!(xechat::services::memory::get_pipeline().is_some());
}

#[test]
fn test_init_embedder_replaces_previous() {
    // 首次初始化
    assert!(xechat::services::embedder::init_embedder(Arc::new(DummyEmbedder)).is_ok());
    let e1 = xechat::services::embedder::get_embedder();
    assert!(e1.is_some());
    assert_eq!(e1.as_ref().unwrap().name(), "dummy");

    // 重复初始化不应报错
    assert!(xechat::services::embedder::init_embedder(Arc::new(DummyEmbedder)).is_ok());
    let e2 = xechat::services::embedder::get_embedder();
    assert!(e2.is_some());
}

#[test]
fn test_raw_turn_struct() {
    use xechat::services::vector_store::lancedb_store::RawTurn;

    let turn = RawTurn {
        id: "test-id".to_string(),
        conversation_id: "conv-1".to_string(),
        user_message_id: "um-1".to_string(),
        assistant_message_id: "am-1".to_string(),
        turn_index: 0,
        user_content: "你好".to_string(),
        assistant_content: "你好！".to_string(),
        timestamp: "2024-01-01T00:00:00+00:00".to_string(),
    };

    assert_eq!(turn.id, "test-id");
    assert_eq!(turn.user_content, "你好");
    assert_eq!(turn.assistant_content, "你好！");
}
