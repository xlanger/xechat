use xechat::models::ai::ChatMessage;
use xechat::services::ai::streaming::{compress_messages, estimate_tokens, extract_error_from_body};

#[test]
fn test_estimate_tokens_empty() {
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn test_estimate_tokens_ascii() {
    let text = "Hello, world!";
    let tokens = estimate_tokens(text);
    assert!(tokens > 0);
    assert!(tokens <= text.len());
}

#[test]
fn test_estimate_tokens_chinese() {
    let text = "你好世界";
    let tokens = estimate_tokens(text);
    assert_eq!(tokens, 2);
}

#[test]
fn test_compress_messages_no_compression_needed() {
    let msgs = vec![
        ChatMessage { role: "user".into(), content: "Hello".into() },
        ChatMessage { role: "assistant".into(), content: "Hi".into() },
    ];
    let result = compress_messages(&msgs, 100, true);
    assert_eq!(result.len(), 2);
}

#[test]
fn test_compress_messages_auto_off() {
    let msgs: Vec<ChatMessage> = (0..10)
        .map(|i| ChatMessage { role: "user".into(), content: format!("msg{}", i) })
        .collect();
    let result = compress_messages(&msgs, 1, false);
    assert_eq!(result.len(), 10);
}

#[test]
fn test_compress_messages_compresses() {
    let msgs: Vec<ChatMessage> = (0..10)
        .map(|_i| ChatMessage {
            role: "user".into(),
            content: "A".repeat(1000),
        })
        .collect();
    let result = compress_messages(&msgs, 100, true);
    assert!(result.len() <= 10);
    assert!(result.len() >= 4);
}

#[test]
fn test_compress_messages_empty() {
    let msgs: Vec<ChatMessage> = vec![];
    let result = compress_messages(&msgs, 100, true);
    assert_eq!(result.len(), 0);
}

#[test]
fn test_compress_messages_min_keep() {
    let msgs: Vec<ChatMessage> = (0..10)
        .map(|_i| ChatMessage {
            role: "user".into(),
            content: "X".repeat(5000),
        })
        .collect();
    let result = compress_messages(&msgs, 1, true);
    assert_eq!(result.len(), 4);
}

#[test]
fn test_extract_error_from_body_valid() {
    let body = r#"{"error":{"message":"Rate limit exceeded"}}"#;
    let result = extract_error_from_body(body);
    assert_eq!(result, Some("Rate limit exceeded".into()));
}

#[test]
fn test_extract_error_from_body_invalid() {
    let result = extract_error_from_body("plain text");
    assert_eq!(result, None);
}
