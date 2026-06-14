use xechat::services::ai::streaming::process_stream_chunk;
use xechat::models::ai::StreamEvent;

// ── process_stream_chunk ──────────────────────────────────────────

#[test]
fn test_process_stream_chunk_done() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let chunk = b"data: [DONE]\n\n";
    let mut buffer = String::new();
    let result = process_stream_chunk(chunk, &mut buffer, &tx);
    assert!(result);
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
}

#[test]
fn test_process_stream_chunk_data() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let data = r#"{"id":"1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#;
    let chunk = format!("data: {}\n\n", data);
    let mut buffer = String::new();
    let result = process_stream_chunk(chunk.as_bytes(), &mut buffer, &tx);
    assert!(!result);
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Chunk(s) if s == "Hi"));
}

#[test]
fn test_process_stream_chunk_incomplete_line() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    // An incomplete SSE line (no trailing newline) - "data: incom" will be
    // parsed as a data line with value "incom", which is not valid JSON,
    // so it will be silently ignored by handle_sse_data.
    let chunk = b"data: incom";
    let mut buffer = String::new();
    let result = process_stream_chunk(chunk, &mut buffer, &tx);
    // Should not terminate the stream
    assert!(!result);
}
