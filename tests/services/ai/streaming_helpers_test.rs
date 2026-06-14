use xechat::models::ai::StreamEvent;
use xechat::models::error::AppError;
use xechat::services::ai::streaming::process_sse_lines;

// ── process_sse_lines ──────────────────────────────────────────

#[test]
fn test_process_sse_lines_data_done() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let lines: Vec<String> = vec!["data: [DONE]".to_string()];
    let mut buffer = String::new();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(result, "Should return true for [DONE]");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
    assert!(buffer.is_empty(), "Buffer should be empty after [DONE]");
}

#[test]
fn test_process_sse_lines_data_chunk() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let data = r#"{"id":"1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#;
    let lines: Vec<String> = vec![format!("data: {}", data)];
    let mut buffer = String::new();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(!result, "Should return false for valid chunk");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Chunk(s) if s == "Hi"));
}

#[test]
fn test_process_sse_lines_metadata_lines_ignored() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let lines: Vec<String> = vec![
        "event: message".to_string(),
        "id: 123".to_string(),
        "retry: 5000".to_string(),
        "".to_string(),
    ];
    let mut buffer = String::new();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(!result, "Should return false for metadata-only lines");
    assert!(buffer.is_empty(), "Buffer should be empty for metadata lines");
}

#[test]
fn test_process_sse_lines_incomplete_last_line_preserved() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    // Last line is not a data line, not metadata, and not empty → should be preserved in buffer
    let lines: Vec<String> = vec![
        "data: [DONE]".to_string(),
        "incomplete_li".to_string(),
    ];
    let mut buffer = String::new();

    // Note: [DONE] is first, so it returns true before reaching the incomplete line
    let result = process_sse_lines(&lines, &tx, &mut buffer);
    assert!(result);
}

#[test]
fn test_process_sse_lines_incomplete_line_buffered() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    // Only an incomplete line (not data, not metadata, not empty, and is last)
    let lines: Vec<String> = vec![
        "incomplete_li".to_string(),
    ];
    let mut buffer = String::new();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(!result, "Should return false for incomplete line");
    assert_eq!(buffer, "incomplete_li\n", "Incomplete line should be preserved in buffer");
}

#[test]
fn test_process_sse_lines_error_response() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let data = r#"{"error":{"message":"Rate limit exceeded"}}"#;
    let lines: Vec<String> = vec![format!("data: {}", data)];
    let mut buffer = String::new();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(result, "Should return true for error response");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Error(AppError::Api { .. })));
}

#[test]
fn test_process_sse_lines_mixed_lines() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let chunk_data = r#"{"id":"1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
    let lines: Vec<String> = vec![
        "event: message".to_string(),
        format!("data: {}", chunk_data),
        "id: 1".to_string(),
        "data: [DONE]".to_string(),
    ];
    let mut buffer = String::new();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(result, "Should return true when [DONE] encountered");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Chunk(s) if s == "Hello"));
    let done_event = rx.try_recv().unwrap();
    assert!(matches!(done_event, StreamEvent::Complete));
}

#[test]
fn test_process_sse_lines_empty_lines() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let lines: Vec<String> = vec![
        "".to_string(),
        "".to_string(),
    ];
    let mut buffer = String::new();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(!result, "Should return false for empty lines");
    assert!(buffer.is_empty(), "Buffer should be empty for empty lines");
}

#[test]
fn test_process_sse_lines_buffer_cleared_before_processing() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let lines: Vec<String> = vec!["data: [DONE]".to_string()];
    let mut buffer = "old content".to_string();

    let result = process_sse_lines(&lines, &tx, &mut buffer);

    assert!(result);
    assert!(buffer.is_empty(), "Buffer should be cleared before processing");
}
