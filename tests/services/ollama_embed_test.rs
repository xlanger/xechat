use xechat::services::ollama::embed::OllamaEmbedder;

// ── extract_dimension ───────────────────────────────────────────────

#[test]
fn test_extract_dimension_valid_response() {
    let body = serde_json::json!({
        "model": "nomic-embed-text",
        "embeddings": [[0.1, 0.2, 0.3, 0.4, 0.5]]
    });
    let dim = OllamaEmbedder::extract_dimension(&body);
    assert_eq!(dim, 5);
}

#[test]
fn test_extract_dimension_large_embedding() {
    let embedding: Vec<f64> = (0..768).map(|i| i as f64 / 768.0).collect();
    let body = serde_json::json!({
        "embeddings": [embedding]
    });
    let dim = OllamaEmbedder::extract_dimension(&body);
    assert_eq!(dim, 768);
}

#[test]
fn test_extract_dimension_empty_embeddings_array() {
    let body = serde_json::json!({
        "embeddings": []
    });
    let dim = OllamaEmbedder::extract_dimension(&body);
    assert_eq!(dim, 768); // fallback default
}

#[test]
fn test_extract_dimension_missing_embeddings() {
    let body = serde_json::json!({
        "model": "nomic-embed-text"
    });
    let dim = OllamaEmbedder::extract_dimension(&body);
    assert_eq!(dim, 768); // fallback default
}

#[test]
fn test_extract_dimension_empty_inner_array() {
    let body = serde_json::json!({
        "embeddings": [[]]
    });
    let dim = OllamaEmbedder::extract_dimension(&body);
    assert_eq!(dim, 0);
}

#[test]
fn test_extract_dimension_multiple_embeddings() {
    let body = serde_json::json!({
        "embeddings": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]
    });
    let dim = OllamaEmbedder::extract_dimension(&body);
    assert_eq!(dim, 3); // Uses first embedding's length
}

// ── parse_embeddings ────────────────────────────────────────────────

#[test]
fn test_parse_embeddings_valid_response() {
    let body = serde_json::json!({
        "embeddings": [[0.1, 0.2, 0.3], [0.4, 0.5, 0.6]]
    });
    let result = OllamaEmbedder::parse_embeddings(&body);

    assert!(result.is_ok());
    let embeddings = result.unwrap();
    assert_eq!(embeddings.len(), 2);
    assert_eq!(embeddings[0].len(), 3);
    assert!((embeddings[0][0] - 0.1).abs() < 0.001);
}

#[test]
fn test_parse_embeddings_single_embedding() {
    let body = serde_json::json!({
        "embeddings": [[0.1, 0.2, 0.3]]
    });
    let result = OllamaEmbedder::parse_embeddings(&body);

    assert!(result.is_ok());
    let embeddings = result.unwrap();
    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].len(), 3);
}

#[test]
fn test_parse_embeddings_empty_array() {
    let body = serde_json::json!({
        "embeddings": []
    });
    let result = OllamaEmbedder::parse_embeddings(&body);

    assert!(result.is_ok());
    let embeddings = result.unwrap();
    assert!(embeddings.is_empty());
}

#[test]
fn test_parse_embeddings_missing_embeddings() {
    let body = serde_json::json!({
        "model": "nomic-embed-text"
    });
    let result = OllamaEmbedder::parse_embeddings(&body);

    assert!(result.is_err());
}

#[test]
fn test_parse_embeddings_invalid_entry() {
    let body = serde_json::json!({
        "embeddings": ["not an array"]
    });
    let result = OllamaEmbedder::parse_embeddings(&body);

    assert!(result.is_err());
}

#[test]
fn test_parse_embeddings_invalid_float() {
    let body = serde_json::json!({
        "embeddings": [["not a number"]]
    });
    let result = OllamaEmbedder::parse_embeddings(&body);

    assert!(result.is_err());
}

#[test]
fn test_parse_embeddings_large_response() {
    let embedding: Vec<f64> = (0..768).map(|i| i as f64 / 768.0).collect();
    let body = serde_json::json!({
        "embeddings": [embedding]
    });
    let result = OllamaEmbedder::parse_embeddings(&body);

    assert!(result.is_ok());
    let embeddings = result.unwrap();
    assert_eq!(embeddings[0].len(), 768);
}
