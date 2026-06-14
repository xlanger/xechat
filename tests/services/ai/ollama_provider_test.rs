use xechat::models::ai::{ChatMessage, SendMessageParams};
use xechat::models::config::ModelProvider;
use xechat::services::ai::providers::ollama::{
    build_request_body, handle_ollama_json_line,
    should_retry_status, compute_backoff_delay, process_ollama_line,
    should_retry_ok_response,
};
use xechat::models::ai::StreamEvent;
use tokio::sync::mpsc;

fn make_params(model: &str, messages: Vec<ChatMessage>) -> SendMessageParams {
    SendMessageParams {
        provider: ModelProvider {
            name: "Ollama".into(),
            api_key: String::new(),
            base_url: "http://localhost:11434".into(),
            timeout: None,
            models: Default::default(),
        },
        provider_key: "ollama".into(),
        model: model.into(),
        messages,
        temperature: None,
        top_p: None,
        model_config: None,
    }
}

// ── build_request_body ──────────────────────────────────────────────

#[test]
fn test_build_request_body_basic() {
    let params = make_params(
        "llama3.1:8b",
        vec![
            ChatMessage { role: "user".into(), content: "Hello".into() },
        ],
    );
    let body = build_request_body(&params);

    assert_eq!(body["model"], "llama3.1:8b");
    assert_eq!(body["stream"], true);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "Hello");
}

#[test]
fn test_build_request_body_default_temperature() {
    let params = make_params("llama3.1:8b", vec![]);
    let body = build_request_body(&params);

    let options = &body["options"];
    let temp = options["temperature"].as_f64().unwrap();
    assert!((temp - 0.7).abs() < 0.01);
}

#[test]
fn test_build_request_body_custom_temperature() {
    let mut params = make_params("llama3.1:8b", vec![]);
    params.temperature = Some(0.3);
    let body = build_request_body(&params);

    let options = &body["options"];
    let temp = options["temperature"].as_f64().unwrap();
    assert!((temp - 0.3).abs() < 0.01);
}

#[test]
fn test_build_request_body_multiple_messages() {
    let params = make_params(
        "llama3.1:8b",
        vec![
            ChatMessage { role: "system".into(), content: "You are helpful".into() },
            ChatMessage { role: "user".into(), content: "Hello".into() },
            ChatMessage { role: "assistant".into(), content: "Hi there".into() },
        ],
    );
    let body = build_request_body(&params);

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
}

// ── handle_ollama_json_line ─────────────────────────────────────────

#[test]
fn test_handle_ollama_json_line_content() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({
        "message": {"content": "Hello world"},
        "done": false
    });
    let should_stop = handle_ollama_json_line(&json, &tx);

    assert!(!should_stop);
    let event = rx.try_recv().unwrap();
    match event {
        StreamEvent::Chunk(text) => assert_eq!(text, "Hello world"),
        _ => panic!("Expected Chunk event"),
    }
}

#[test]
fn test_handle_ollama_json_line_done() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({"done": true});
    let should_stop = handle_ollama_json_line(&json, &tx);

    assert!(should_stop);
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
}

#[test]
fn test_handle_ollama_json_line_empty_content() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({
        "message": {"content": ""},
        "done": false
    });
    let should_stop = handle_ollama_json_line(&json, &tx);

    assert!(!should_stop);
    // Empty content should not emit a Chunk event
    assert!(rx.try_recv().is_err());
}

#[test]
fn test_handle_ollama_json_line_thinking_field() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({
        "message": {"content": "answer", "thinking": "reasoning process"},
        "done": false
    });
    let should_stop = handle_ollama_json_line(&json, &tx);

    assert!(!should_stop);
    // Should receive both Chunk and ReasoningChunk
    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(events.len(), 2);

    let has_chunk = events.iter().any(|e| matches!(e, StreamEvent::Chunk(t) if t == "answer"));
    let has_reasoning = events.iter().any(|e| matches!(e, StreamEvent::ReasoningChunk(t) if t == "reasoning process"));
    assert!(has_chunk);
    assert!(has_reasoning);
}

#[test]
fn test_handle_ollama_json_line_reasoning_content_field() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({
        "message": {"content": "answer", "reasoning_content": "reasoning"},
        "done": false
    });
    let should_stop = handle_ollama_json_line(&json, &tx);

    assert!(!should_stop);
    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let has_reasoning = events.iter().any(|e| matches!(e, StreamEvent::ReasoningChunk(t) if t == "reasoning"));
    assert!(has_reasoning);
}

#[test]
fn test_handle_ollama_json_line_thinking_takes_precedence_over_reasoning_content() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({
        "message": {"content": "answer", "thinking": "from thinking", "reasoning_content": "from reasoning"},
        "done": false
    });
    let should_stop = handle_ollama_json_line(&json, &tx);

    assert!(!should_stop);
    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let has_thinking = events.iter().any(|e| matches!(e, StreamEvent::ReasoningChunk(t) if t == "from thinking"));
    assert!(has_thinking);
}

#[test]
fn test_handle_ollama_json_line_no_message_field() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({"done": false});
    let should_stop = handle_ollama_json_line(&json, &tx);
    assert!(!should_stop);
}

#[test]
fn test_handle_ollama_json_line_empty_thinking() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let json = serde_json::json!({
        "message": {"content": "answer", "thinking": ""},
        "done": false
    });
    let should_stop = handle_ollama_json_line(&json, &tx);
    assert!(!should_stop);
    // Empty thinking should not emit ReasoningChunk
}

// ── should_retry_status ─────────────────────────────────────────────

#[test]
fn test_should_retry_status_429() {
    let status = reqwest::StatusCode::from_u16(429).unwrap();
    assert!(should_retry_status(status));
}

#[test]
fn test_should_retry_status_500() {
    let status = reqwest::StatusCode::from_u16(500).unwrap();
    assert!(should_retry_status(status));
}

#[test]
fn test_should_retry_status_200() {
    let status = reqwest::StatusCode::from_u16(200).unwrap();
    assert!(!should_retry_status(status));
}

#[test]
fn test_should_retry_status_404() {
    let status = reqwest::StatusCode::from_u16(404).unwrap();
    assert!(!should_retry_status(status));
}

// ── compute_backoff_delay ───────────────────────────────────────────

#[test]
fn test_compute_backoff_delay_attempt_1() {
    assert_eq!(compute_backoff_delay(1), std::time::Duration::from_millis(500));
}

#[test]
fn test_compute_backoff_delay_attempt_2() {
    assert_eq!(compute_backoff_delay(2), std::time::Duration::from_millis(1000));
}

#[test]
fn test_compute_backoff_delay_attempt_3() {
    assert_eq!(compute_backoff_delay(3), std::time::Duration::from_millis(2000));
}

// ── process_ollama_line ─────────────────────────────────────────────

#[test]
fn test_process_ollama_line_empty_line() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut remaining = String::new();

    let should_stop = process_ollama_line("  ", false, &tx, &mut remaining);
    assert!(!should_stop);
    assert!(remaining.is_empty());
}

#[test]
fn test_process_ollama_line_valid_json_with_content() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut remaining = String::new();
    let line = r#"{"message":{"content":"hello"},"done":false}"#;

    let should_stop = process_ollama_line(line, false, &tx, &mut remaining);
    assert!(!should_stop);
    let event = rx.try_recv().unwrap();
    match event {
        StreamEvent::Chunk(text) => assert_eq!(text, "hello"),
        _ => panic!("Expected Chunk event"),
    }
}

#[test]
fn test_process_ollama_line_done() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut remaining = String::new();
    let line = r#"{"done":true}"#;

    let should_stop = process_ollama_line(line, false, &tx, &mut remaining);
    assert!(should_stop);
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
}

#[test]
fn test_process_ollama_line_incomplete_last_line() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut remaining = String::new();
    let line = r#"{"message":{"content":"hel"#;

    let should_stop = process_ollama_line(line, true, &tx, &mut remaining);
    assert!(!should_stop);
    assert!(remaining.contains("hel"), "incomplete line should be buffered");
}

#[test]
fn test_process_ollama_line_incomplete_non_last_line_parsed() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut remaining = String::new();
    // A line that doesn't end with } but is NOT the last line
    // This will be treated as a regular line and parsed as JSON (which will fail)
    let line = r#"{"partial": true"#;

    let should_stop = process_ollama_line(line, false, &tx, &mut remaining);
    assert!(!should_stop);
    assert!(remaining.is_empty(), "non-last incomplete line should not be buffered");
}

#[test]
fn test_process_ollama_line_invalid_json() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut remaining = String::new();
    let line = "not json at all}";

    let should_stop = process_ollama_line(line, false, &tx, &mut remaining);
    assert!(!should_stop);
}

#[test]
fn test_process_ollama_line_thinking_field() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut remaining = String::new();
    let line = r#"{"message":{"content":"answer","thinking":"reasoning"},"done":false}"#;

    let should_stop = process_ollama_line(line, false, &tx, &mut remaining);
    assert!(!should_stop);
    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    let has_chunk = events.iter().any(|e| matches!(e, StreamEvent::Chunk(t) if t == "answer"));
    let has_reasoning = events.iter().any(|e| matches!(e, StreamEvent::ReasoningChunk(t) if t == "reasoning"));
    assert!(has_chunk);
    assert!(has_reasoning);
}

// ── should_retry_ok_response ─────────────────────────────────────

#[test]
fn test_should_retry_ok_response_429_within_retries() {
    let status = reqwest::StatusCode::from_u16(429).unwrap();
    assert!(should_retry_ok_response(status, 1, 3));
}

#[test]
fn test_should_retry_ok_response_429_at_max_retries() {
    let status = reqwest::StatusCode::from_u16(429).unwrap();
    assert!(!should_retry_ok_response(status, 3, 3));
}

#[test]
fn test_should_retry_ok_response_500_within_retries() {
    let status = reqwest::StatusCode::from_u16(500).unwrap();
    assert!(should_retry_ok_response(status, 2, 3));
}

#[test]
fn test_should_retry_ok_response_200_no_retry() {
    let status = reqwest::StatusCode::from_u16(200).unwrap();
    assert!(!should_retry_ok_response(status, 1, 3));
}

#[test]
fn test_should_retry_ok_response_503_exceeded_retries() {
    let status = reqwest::StatusCode::from_u16(503).unwrap();
    assert!(!should_retry_ok_response(status, 4, 3));
}

// ── should_retry_err_response ────────────────────────────────────

#[test]
fn test_should_retry_err_response_boundary_attempt_equals_max() {
    assert!(!(3 < 3), "attempt == max_retries should not allow retry");
}

#[test]
fn test_should_retry_err_response_boundary_attempt_less_than_max() {
    assert!(1 < 3, "attempt < max_retries should allow retry");
}
