use xechat::services::ollama::probe::classify_model;

#[test]
fn test_classify_embed_model() {
    assert_eq!(classify_model("nomic-embed-text"), "embed");
    assert_eq!(classify_model("jina-embeddings-v2"), "embed");
    assert_eq!(classify_model("gte-qwen2"), "embed");
}

#[test]
fn test_classify_chat_model() {
    assert_eq!(classify_model("qwen3:8b"), "chat");
    assert_eq!(classify_model("llama3.1:8b"), "chat");
    assert_eq!(classify_model("deepseek-r1:7b"), "chat");
}

#[test]
fn test_classify_embed_model_with_version() {
    assert_eq!(classify_model("nomic-embed-text:v1.5"), "embed");
}
