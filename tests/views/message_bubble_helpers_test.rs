use xechat::views::conversation::message_bubble::resolve_reasoning_text;

// ── resolve_reasoning_text ──────────────────────────────────────

#[test]
fn test_resolve_reasoning_text_streaming_takes_priority() {
    let streaming = Some("streaming reasoning".to_string());
    let message = Some("message reasoning".to_string());
    assert_eq!(resolve_reasoning_text(&streaming, &message), "streaming reasoning");
}

#[test]
fn test_resolve_reasoning_text_fallback_to_message() {
    let streaming = None;
    let message = Some("message reasoning".to_string());
    assert_eq!(resolve_reasoning_text(&streaming, &message), "message reasoning");
}

#[test]
fn test_resolve_reasoning_text_both_none() {
    let streaming = None;
    let message = None;
    assert_eq!(resolve_reasoning_text(&streaming, &message), "");
}

#[test]
fn test_resolve_reasoning_text_streaming_empty_string() {
    // Empty string in streaming is still Some, so it takes priority
    let streaming = Some("".to_string());
    let message = Some("message reasoning".to_string());
    assert_eq!(resolve_reasoning_text(&streaming, &message), "");
}

#[test]
fn test_resolve_reasoning_text_message_empty_string() {
    let streaming = None;
    let message = Some("".to_string());
    assert_eq!(resolve_reasoning_text(&streaming, &message), "");
}
