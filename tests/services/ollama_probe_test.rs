use xechat::services::ollama::probe::{classify_model, populate_models_from_json, apply_preferred_models};
use xechat::services::ollama::{OllamaConfig, OllamaStatus};

// ── populate_models_from_json ───────────────────────────────────────

#[test]
fn test_populate_models_empty_json() {
    let mut status = OllamaStatus::default();
    populate_models_from_json(&mut status, serde_json::json!({}));

    assert!(status.embed_model.is_none());
    assert!(status.chat_model.is_none());
}

#[test]
fn test_populate_models_empty_array() {
    let mut status = OllamaStatus::default();
    populate_models_from_json(&mut status, serde_json::json!({"models": []}));

    assert!(status.embed_model.is_none());
    assert!(status.chat_model.is_none());
}

#[test]
fn test_populate_models_embed_model() {
    let mut status = OllamaStatus::default();
    populate_models_from_json(&mut status, serde_json::json!({
        "models": [{"name": "qwen3-embedding:0.6b"}]
    }));

    assert_eq!(status.embed_model, Some("qwen3-embedding:0.6b".to_string()));
    assert!(status.chat_model.is_none());
}

#[test]
fn test_populate_models_chat_model() {
    let mut status = OllamaStatus::default();
    populate_models_from_json(&mut status, serde_json::json!({
        "models": [{"name": "llama3.1:8b"}]
    }));

    assert!(status.embed_model.is_none());
    assert_eq!(status.chat_model, Some("llama3.1:8b".to_string()));
}

#[test]
fn test_populate_models_mixed_models() {
    let mut status = OllamaStatus::default();
    populate_models_from_json(&mut status, serde_json::json!({
        "models": [
            {"name": "llama3.1:8b"},
            {"name": "qwen3-embedding:0.6b"},
            {"name": "deepseek-r1:7b"},
        ]
    }));

    assert_eq!(status.embed_model, Some("qwen3-embedding:0.6b".to_string()));
    assert_eq!(status.chat_model, Some("llama3.1:8b".to_string()));
}

#[test]
fn test_populate_models_only_first_embed_and_chat() {
    let mut status = OllamaStatus::default();
    populate_models_from_json(&mut status, serde_json::json!({
        "models": [
            {"name": "qwen3-embedding:0.6b"},
            {"name": "qwen3-embedding:latest"},
            {"name": "llama3.1:8b"},
            {"name": "qwen3:8b"},
        ]
    }));

    // Should only fill the first embed and first chat
    assert_eq!(status.embed_model, Some("qwen3-embedding:0.6b".to_string()));
    assert_eq!(status.chat_model, Some("llama3.1:8b".to_string()));
}

#[test]
fn test_populate_models_already_set_not_overridden() {
    let mut status = OllamaStatus {
        embed_model: Some("existing-embed".to_string()),
        chat_model: Some("existing-chat".to_string()),
        ..Default::default()
    };

    populate_models_from_json(&mut status, serde_json::json!({
        "models": [{"name": "qwen3-embedding:0.6b"}, {"name": "llama3.1:8b"}]
    }));

    // Already set values should not be overridden
    assert_eq!(status.embed_model, Some("existing-embed".to_string()));
    assert_eq!(status.chat_model, Some("existing-chat".to_string()));
}

#[test]
fn test_populate_models_missing_name_field() {
    let mut status = OllamaStatus::default();
    populate_models_from_json(&mut status, serde_json::json!({
        "models": [{"other": "field"}]
    }));

    // Empty name is classified as "chat" by classify_model
    assert!(status.embed_model.is_none());
    assert_eq!(status.chat_model, Some("".to_string()));
}

// ── apply_preferred_models ──────────────────────────────────────────

#[test]
fn test_apply_preferred_models_no_preferences() {
    let config = OllamaConfig {
        host: "http://localhost:11434".to_string(),
        preferred_embed: None,
        preferred_chat: None,
    };
    let mut status = OllamaStatus {
        host: "http://localhost:11434".to_string(),
        available: true,
        version: "0.1.0".to_string(),
        embed_model: Some("qwen3-embedding:0.6b".to_string()),
        chat_model: Some("llama3.1:8b".to_string()),
    };

    apply_preferred_models(&config, &mut status);

    assert_eq!(status.embed_model, Some("qwen3-embedding:0.6b".to_string()));
    assert_eq!(status.chat_model, Some("llama3.1:8b".to_string()));
}

#[test]
fn test_apply_preferred_models_override_embed() {
    let config = OllamaConfig {
        host: "http://localhost:11434".to_string(),
        preferred_embed: Some("my-custom-embed".to_string()),
        preferred_chat: None,
    };
    let mut status = OllamaStatus {
        host: "http://localhost:11434".to_string(),
        available: true,
        version: "0.1.0".to_string(),
        embed_model: Some("qwen3-embedding:0.6b".to_string()),
        chat_model: Some("llama3.1:8b".to_string()),
    };

    apply_preferred_models(&config, &mut status);

    assert_eq!(status.embed_model, Some("my-custom-embed".to_string()));
    assert_eq!(status.chat_model, Some("llama3.1:8b".to_string()));
}

#[test]
fn test_apply_preferred_models_override_chat() {
    let config = OllamaConfig {
        host: "http://localhost:11434".to_string(),
        preferred_embed: None,
        preferred_chat: Some("my-custom-chat".to_string()),
    };
    let mut status = OllamaStatus {
        host: "http://localhost:11434".to_string(),
        available: true,
        version: "0.1.0".to_string(),
        embed_model: Some("qwen3-embedding:0.6b".to_string()),
        chat_model: Some("llama3.1:8b".to_string()),
    };

    apply_preferred_models(&config, &mut status);

    assert_eq!(status.embed_model, Some("qwen3-embedding:0.6b".to_string()));
    assert_eq!(status.chat_model, Some("my-custom-chat".to_string()));
}

#[test]
fn test_apply_preferred_models_override_both() {
    let config = OllamaConfig {
        host: "http://localhost:11434".to_string(),
        preferred_embed: Some("custom-embed".to_string()),
        preferred_chat: Some("custom-chat".to_string()),
    };
    let mut status = OllamaStatus {
        host: "http://localhost:11434".to_string(),
        available: true,
        version: "0.1.0".to_string(),
        embed_model: None,
        chat_model: None,
    };

    apply_preferred_models(&config, &mut status);

    assert_eq!(status.embed_model, Some("custom-embed".to_string()));
    assert_eq!(status.chat_model, Some("custom-chat".to_string()));
}

// ── classify_model (additional edge cases) ──────────────────────────

#[test]
fn test_classify_model_qwen3_embedding() {
    assert_eq!(classify_model("qwen3-embedding:0.6b"), "embed");
}

#[test]
fn test_classify_model_qwen3_embed_variant() {
    assert_eq!(classify_model("qwen3-embedding:latest"), "embed");
}

#[test]
fn test_classify_model_case_insensitive() {
    assert_eq!(classify_model("QWEN3-EMBEDDING:0.6B"), "embed");
}
