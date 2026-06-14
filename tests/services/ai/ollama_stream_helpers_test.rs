use xechat::services::ai::providers::ollama::process_ollama_chunk;
use xechat::models::ai::StreamEvent;
use tokio::sync::mpsc;

// ── process_ollama_chunk ────────────────────────────────────────

#[test]
fn test_process_ollama_chunk_single_line_with_content() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let chunk = r#"{"message":{"content":"hello"},"done":false}"#.as_bytes();

    let should_stop = process_ollama_chunk(chunk, &mut buffer, &tx);
    assert!(!should_stop);
    let event = rx.try_recv().unwrap();
    match event {
        StreamEvent::Chunk(text) => assert_eq!(text, "hello"),
        _ => panic!("Expected Chunk event"),
    }
}

#[test]
fn test_process_ollama_chunk_done() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let chunk = r#"{"done":true}"#.as_bytes();

    let should_stop = process_ollama_chunk(chunk, &mut buffer, &tx);
    assert!(should_stop);
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
}

#[test]
fn test_process_ollama_chunk_incomplete_line() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let chunk = r#"{"message":{"content":"hel"#.as_bytes();

    let should_stop = process_ollama_chunk(chunk, &mut buffer, &tx);
    assert!(!should_stop);
    // The incomplete line should remain in the buffer
    assert!(!buffer.is_empty());
}

#[test]
fn test_process_ollama_chunk_multiple_lines() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let chunk = r#"{"message":{"content":"hello"},"done":false}
{"message":{"content":" world"},"done":false}
"#.as_bytes();

    let should_stop = process_ollama_chunk(chunk, &mut buffer, &tx);
    assert!(!should_stop);

    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(events.len(), 2);
}

#[test]
fn test_process_ollama_chunk_empty_chunk() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let chunk = b"";

    let should_stop = process_ollama_chunk(chunk, &mut buffer, &tx);
    assert!(!should_stop);
    assert!(buffer.is_empty());
}

#[test]
fn test_process_ollama_chunk_continuation_from_buffer() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = r#"{"message":{"content":"hel"#.to_string();
    let chunk = b"lo\"},\"done\":false}\n";

    let should_stop = process_ollama_chunk(chunk, &mut buffer, &tx);
    assert!(!should_stop);

    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(events.len(), 1);
    match &events[0] {
        StreamEvent::Chunk(text) => assert_eq!(text, "hello"),
        _ => panic!("Expected Chunk event"),
    }
}
