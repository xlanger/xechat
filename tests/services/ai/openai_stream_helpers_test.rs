use xechat::services::ai::providers::openai::process_responses_chunk;
use xechat::models::ai::StreamEvent;
use tokio::sync::mpsc;

// ── process_responses_chunk ─────────────────────────────────────

#[test]
fn test_process_responses_chunk_text_delta() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let mut current_event = "response.output_text.delta".to_string();
    let chunk = b"data: {\"delta\":\"hello\"}\n\n";

    let should_stop = process_responses_chunk(chunk, &mut buffer, &mut current_event, &tx);
    assert!(!should_stop);

    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(events.len(), 1);
    match &events[0] {
        StreamEvent::Chunk(text) => assert_eq!(text, "hello"),
        _ => panic!("Expected Chunk event"),
    }
}

#[test]
fn test_process_responses_chunk_completed() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let mut current_event = "response.completed".to_string();
    let chunk = b"data: {}\n\n";

    let should_stop = process_responses_chunk(chunk, &mut buffer, &mut current_event, &tx);
    assert!(should_stop);

    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
}

#[test]
fn test_process_responses_chunk_done_signal() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let mut current_event = String::new();
    let chunk = b"data: [DONE]\n\n";

    let should_stop = process_responses_chunk(chunk, &mut buffer, &mut current_event, &tx);
    assert!(should_stop);

    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
}

#[test]
fn test_process_responses_chunk_incomplete_line() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let mut current_event = "response.output_text.delta".to_string();
    // An incomplete line that doesn't start with "data:" or "event:" will be buffered
    let chunk = b"da";

    let should_stop = process_responses_chunk(chunk, &mut buffer, &mut current_event, &tx);
    assert!(!should_stop);
    // The incomplete line should remain in the buffer (it's the last line and doesn't match any prefix)
    assert!(!buffer.is_empty());
}

#[test]
fn test_process_responses_chunk_event_type_setting() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let mut current_event = String::new();
    // First set the event type, then send data
    let chunk = b"event: response.output_text.delta\ndata: {\"delta\":\"hello\"}\n\n";

    let should_stop = process_responses_chunk(chunk, &mut buffer, &mut current_event, &tx);
    assert!(!should_stop);

    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(events.len(), 1);
    match &events[0] {
        StreamEvent::Chunk(text) => assert_eq!(text, "hello"),
        _ => panic!("Expected Chunk event"),
    }
}

#[test]
fn test_process_responses_chunk_reasoning_delta() {
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let mut current_event = "response.reasoning.delta".to_string();
    let chunk = b"data: {\"delta\":\"thinking...\"}\n\n";

    let should_stop = process_responses_chunk(chunk, &mut buffer, &mut current_event, &tx);
    assert!(!should_stop);

    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(events.len(), 1);
    match &events[0] {
        StreamEvent::ReasoningChunk(text) => assert_eq!(text, "thinking..."),
        _ => panic!("Expected ReasoningChunk event"),
    }
}

#[test]
fn test_process_responses_chunk_empty_chunk() {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut buffer = String::new();
    let mut current_event = String::new();
    let chunk = b"";

    let should_stop = process_responses_chunk(chunk, &mut buffer, &mut current_event, &tx);
    assert!(!should_stop);
    assert!(buffer.is_empty());
}
