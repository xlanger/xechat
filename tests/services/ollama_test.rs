use xechat::services::ollama::probe::classify_model;

#[test]
fn test_classify_embed_model() {
    assert_eq!(classify_model("qwen3-embedding:0.6b"), "embed");
    assert_eq!(classify_model("qwen3-embedding:latest"), "embed");
    assert_eq!(classify_model("qwen3-embedding:1.7b"), "embed");
}

#[test]
fn test_classify_chat_model() {
    assert_eq!(classify_model("qwen3:8b"), "chat");
    assert_eq!(classify_model("llama3.1:8b"), "chat");
    assert_eq!(classify_model("deepseek-r1:7b"), "chat");
}

#[test]
fn test_classify_embed_model_with_version() {
    assert_eq!(classify_model("qwen3-embedding:0.6b"), "embed");
}
