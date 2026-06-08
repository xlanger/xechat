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
