use xechat::services::ollama::probe::{filter_models_by_category, model_name_matches, find_model_in_json};

// ── filter_models_by_category ─────────────────────────────────────

#[test]
fn test_filter_models_by_category_embed_by_name() {
    // 无 details 字段时，fallback 到名称匹配（qwen3 + embed）
    let json = serde_json::json!({
        "models": [
            {"name": "qwen3-embedding:0.6b"},
            {"name": "llama3.1:8b"},
            {"name": "qwen3-embedding:4b"},
        ]
    });
    let result = filter_models_by_category(&json, "embed");
    assert_eq!(result, vec!["qwen3-embedding:0.6b", "qwen3-embedding:4b"]);
}

#[test]
fn test_filter_models_by_category_embed_by_details_family() {
    // 有 details.family 字段时，优先使用 family 识别
    let json = serde_json::json!({
        "models": [
            {
                "name": "my-custom-embed-model",
                "details": {"family": "qwen3-embedding", "parameter_size": "0.6B"}
            },
            {"name": "llama3.1:8b"},
        ]
    });
    let result = filter_models_by_category(&json, "embed");
    assert_eq!(result, vec!["my-custom-embed-model"]);
}

#[test]
fn test_filter_models_by_category_chat() {
    let json = serde_json::json!({
        "models": [
            {"name": "qwen3-embedding:0.6b"},
            {"name": "llama3.1:8b"},
        ]
    });
    let result = filter_models_by_category(&json, "chat");
    assert_eq!(result, vec!["llama3.1:8b"]);
}

#[test]
fn test_filter_models_by_category_empty() {
    let json = serde_json::json!({});
    let result = filter_models_by_category(&json, "embed");
    assert!(result.is_empty());
}

#[test]
fn test_filter_models_by_category_no_matching() {
    let json = serde_json::json!({
        "models": [{"name": "llama3.1:8b"}]
    });
    let result = filter_models_by_category(&json, "embed");
    assert!(result.is_empty());
}

#[test]
fn test_filter_models_by_category_non_qwen3_embed_excluded() {
    // 非 Qwen3-Embedding 的嵌入模型应被过滤掉
    let json = serde_json::json!({
        "models": [
            {"name": "nomic-embed-text"},
            {"name": "jina-embeddings-v2"},
            {"name": "bge-base-zh"},
        ]
    });
    let result = filter_models_by_category(&json, "embed");
    assert!(result.is_empty(), "Non-Qwen3 embedding models should be excluded");
}

// ── model_name_matches ────────────────────────────────────────────

#[test]
fn test_model_name_matches_exact() {
    assert!(model_name_matches("qwen3-embedding:0.6b", "qwen3-embedding:0.6b"));
}

#[test]
fn test_model_name_matches_with_tag() {
    assert!(model_name_matches("qwen3-embedding:latest", "qwen3-embedding"));
}

#[test]
fn test_model_name_matches_different_model() {
    assert!(!model_name_matches("llama3.1:8b", "qwen3-embedding"));
}

#[test]
fn test_model_name_matches_partial_prefix() {
    assert!(!model_name_matches("qwen3-embedding-v2", "qwen3-embedding"));
}

// ── find_model_in_json ────────────────────────────────────────────

#[test]
fn test_find_model_in_json_exists() {
    let json = serde_json::json!({
        "models": [
            {"name": "qwen3-embedding:0.6b"},
            {"name": "llama3.1:8b"},
        ]
    });
    assert!(find_model_in_json(&json, "qwen3-embedding:0.6b"));
}

#[test]
fn test_find_model_in_json_not_exists() {
    let json = serde_json::json!({
        "models": [{"name": "llama3.1:8b"}]
    });
    assert!(!find_model_in_json(&json, "qwen3-embedding:0.6b"));
}

#[test]
fn test_find_model_in_json_empty() {
    let json = serde_json::json!({});
    assert!(!find_model_in_json(&json, "qwen3-embedding:0.6b"));
}
