use xechat::models::ai::StreamEvent;
use xechat::models::error::AppError;
use xechat::models::ai::ChatMessage;
use xechat::services::ai::streaming::{compress_messages, estimate_tokens, extract_error_from_body, extract_data_field, is_sse_metadata_or_empty, handle_sse_data};

// ── estimate_tokens ──────────────────────────────────────────────

#[test]
fn test_estimate_tokens_empty_string() {
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn test_estimate_tokens_single_char() {
    // 1 / 3.5 ceil = 1
    assert_eq!(estimate_tokens("a"), 1);
}

#[test]
fn test_estimate_tokens_hello() {
    // "hello" = 5 chars, ceil(5 / 3.5) = 2
    assert_eq!(estimate_tokens("hello"), 2);
}

#[test]
fn test_estimate_tokens_longer_text() {
    let text = "The quick brown fox jumps over the lazy dog";
    let char_count = text.chars().count(); // 43
    let expected = (char_count as f64 / 3.5).ceil() as usize;
    assert_eq!(estimate_tokens(text), expected);
}

#[test]
fn test_estimate_tokens_chinese() {
    // 3 Chinese chars, ceil(3 / 3.5) = 1
    assert_eq!(estimate_tokens("你好吗"), 1);
}

// ── compress_messages ────────────────────────────────────────────

#[test]
fn test_compress_messages_auto_management_off() {
    let msgs: Vec<ChatMessage> = (0..10)
        .map(|i| ChatMessage {
            role: "user".into(),
            content: format!("msg{i}"),
        })
        .collect();
    let result = compress_messages(&msgs, 1, false);
    assert_eq!(result.len(), 10);
}

#[test]
fn test_compress_messages_empty() {
    let msgs: Vec<ChatMessage> = vec![];
    let result = compress_messages(&msgs, 100, true);
    assert!(result.is_empty());
}

#[test]
fn test_compress_messages_under_limit() {
    let msgs = vec![
        ChatMessage {
            role: "user".into(),
            content: "Hello".into(),
        },
        ChatMessage {
            role: "assistant".into(),
            content: "Hi there".into(),
        },
    ];
    let result = compress_messages(&msgs, 10000, true);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "Hello");
    assert_eq!(result[1].content, "Hi there");
}

#[test]
fn test_compress_messages_over_token_limit_removes_from_head() {
    // Each message has 1000 chars => ~286 tokens each
    let msgs: Vec<ChatMessage> = (0..10)
        .map(|i| ChatMessage {
            role: "user".into(),
            content: format!("{i}_{}", "A".repeat(998)),
        })
        .collect();
    // Set a low max_tokens so compression kicks in
    let result = compress_messages(&msgs, 500, true);
    // Should have removed some from the head but keep at least MIN_KEEP_MESSAGES (4)
    assert!(result.len() >= 4);
    assert!(result.len() < 10);
    // Verify the remaining messages are from the tail
    assert!(result.last().unwrap().content.starts_with('9'));
}

#[test]
fn test_compress_messages_never_below_min_keep() {
    // Each message has 5000 chars => ~1429 tokens each
    let msgs: Vec<ChatMessage> = (0..10)
        .map(|_| ChatMessage {
            role: "user".into(),
            content: "X".repeat(5000),
        })
        .collect();
    // Even with max_tokens=1, should keep MIN_KEEP_MESSAGES (4)
    let result = compress_messages(&msgs, 1, true);
    assert_eq!(result.len(), 4);
}

#[test]
fn test_compress_messages_respects_max_context_messages() {
    // Create 30 messages, MAX_CONTEXT_MESSAGES is 20
    let msgs: Vec<ChatMessage> = (0..30)
        .map(|i| ChatMessage {
            role: "user".into(),
            content: format!("msg{i}"),
        })
        .collect();
    let result = compress_messages(&msgs, 100000, true);
    assert_eq!(result.len(), 20);
    // Should keep the last 20 messages (indices 10..29)
    assert_eq!(result[0].content, "msg10");
    assert_eq!(result[19].content, "msg29");
}

#[test]
fn test_compress_messages_single_message_stays_even_if_over_limit() {
    let msgs = vec![ChatMessage {
        role: "user".into(),
        content: "X".repeat(10000),
    }];
    let result = compress_messages(&msgs, 1, true);
    // Single message: len=1 <= MIN_KEEP_MESSAGES(4), so while loop doesn't run
    assert_eq!(result.len(), 1);
}

// ── extract_error_from_body ──────────────────────────────────────

#[test]
fn test_extract_error_from_body_valid_openai_format() {
    let body = r#"{"error":{"message":"Rate limit exceeded"}}"#;
    let result = extract_error_from_body(body);
    assert_eq!(result, Some("Rate limit exceeded".to_string()));
}

#[test]
fn test_extract_error_from_body_no_error_json() {
    let result = extract_error_from_body("plain text without error");
    assert_eq!(result, None);
}

#[test]
fn test_extract_error_from_body_empty() {
    let result = extract_error_from_body("");
    assert_eq!(result, None);
}

#[test]
fn test_extract_error_from_body_error_at_nonzero_offset() {
    let body = r#"some prefix data {"error":{"message":"Model not found"}}"#;
    let result = extract_error_from_body(body);
    assert_eq!(result, Some("Model not found".to_string()));
}

#[test]
fn test_extract_error_from_body_malformed_after_error_key() {
    let body = r#"{"error": not valid json}"#;
    let result = extract_error_from_body(body);
    assert_eq!(result, None);
}

#[test]
fn test_extract_error_from_body_nested_error_with_message() {
    let body = r#"{"error":{"message":"Invalid API key","type":"invalid_request_error","code":"invalid_api_key"}}"#;
    let result = extract_error_from_body(body);
    assert_eq!(result, Some("Invalid API key".to_string()));
}

// ── extract_data_field ──────────────────────────────────────────

#[test]
fn test_extract_data_field_with_prefix() {
    assert_eq!(extract_data_field("data: hello"), Some("hello"));
}

#[test]
fn test_extract_data_field_with_done() {
    assert_eq!(extract_data_field("data: [DONE]"), Some("[DONE]"));
}

#[test]
fn test_extract_data_field_without_prefix() {
    assert_eq!(extract_data_field("event: message"), None);
}

#[test]
fn test_extract_data_field_empty_line() {
    assert_eq!(extract_data_field(""), None);
}

#[test]
fn test_extract_data_field_data_only() {
    // "data:" without trailing space should not match
    assert_eq!(extract_data_field("data:"), None);
}

#[test]
fn test_extract_data_field_data_with_space_only() {
    assert_eq!(extract_data_field("data: "), Some(""));
}

// ── is_sse_metadata_or_empty ────────────────────────────────────

#[test]
fn test_is_sse_metadata_event() {
    assert!(is_sse_metadata_or_empty("event: message"));
}

#[test]
fn test_is_sse_metadata_id() {
    assert!(is_sse_metadata_or_empty("id: 123"));
}

#[test]
fn test_is_sse_metadata_retry() {
    assert!(is_sse_metadata_or_empty("retry: 5000"));
}

#[test]
fn test_is_sse_metadata_empty() {
    assert!(is_sse_metadata_or_empty(""));
}

#[test]
fn test_is_sse_metadata_data_line() {
    assert!(!is_sse_metadata_or_empty("data: hello"));
}

#[test]
fn test_is_sse_metadata_regular_text() {
    assert!(!is_sse_metadata_or_empty("some random text"));
}

// ── handle_sse_data ─────────────────────────────────────────────

#[test]
fn test_handle_sse_data_done() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let result = handle_sse_data("[DONE]", &tx);
    assert!(result, "Should return true for [DONE]");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Complete));
}

#[test]
fn test_handle_sse_data_valid_chunk() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let data = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
    let result = handle_sse_data(data, &tx);
    assert!(!result, "Should return false for valid chunk");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Chunk(s) if s == "Hello"));
}

#[test]
fn test_handle_sse_data_reasoning_chunk() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let data = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"reasoning_content":"thinking..."},"finish_reason":null}]}"#;
    let result = handle_sse_data(data, &tx);
    assert!(!result, "Should return false for reasoning chunk");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::ReasoningChunk(s) if s == "thinking..."));
}

#[test]
fn test_handle_sse_data_error_response() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let data = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
    let result = handle_sse_data(data, &tx);
    assert!(result, "Should return true for error response");
    let event = rx.try_recv().unwrap();
    assert!(matches!(event, StreamEvent::Error(AppError::Api { .. })));
}

#[test]
fn test_handle_sse_data_unrecognized_json() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let data = r#"{"type":"unknown","value":42}"#;
    let result = handle_sse_data(data, &tx);
    assert!(!result, "Should return false for unrecognized JSON");
}

#[test]
fn test_handle_sse_data_invalid_json() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let result = handle_sse_data("not json at all", &tx);
    assert!(!result, "Should return false for non-JSON");
}
