use async_trait::async_trait;
use xechat::services::embedder::Embedder;

struct DummyEmbedder;

#[async_trait]
impl Embedder for DummyEmbedder {
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![0.0; 768]).collect())
    }
    async fn encode_one(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![0.0; 768])
    }
    fn dimension(&self) -> usize {
        768
    }
    fn name(&self) -> &str {
        "dummy"
    }
}

#[test]
fn test_embedder_trait_interface() {
    fn accepts_embedder(_: &dyn Embedder) {}
    let d = DummyEmbedder;
    accepts_embedder(&d);
}

#[tokio::test]
async fn test_dummy_encode_batch() {
    let d = DummyEmbedder;
    let result = d.encode(&["hello", "world"]).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].len(), 768);
}
