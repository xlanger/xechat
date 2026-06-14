use xechat::views::conversation::message_list::{
    compute_scroll_threshold, should_load_older, should_load_newer,
    should_show_streaming, has_real_assistant_message,
};
use xechat::{Message, MessageRole, MessageStatus};

// ── compute_scroll_threshold ──────────────────────────────────────

#[test]
fn test_compute_scroll_threshold_small_container() {
    let threshold = compute_scroll_threshold(100.0);
    assert_eq!(threshold, 80.0); // clamped to THRESHOLD_MIN
}

#[test]
fn test_compute_scroll_threshold_large_container() {
    let threshold = compute_scroll_threshold(2000.0);
    assert_eq!(threshold, 200.0); // clamped to THRESHOLD_MAX
}

#[test]
fn test_compute_scroll_threshold_medium_container() {
    let threshold = compute_scroll_threshold(500.0);
    assert_eq!(threshold, 100.0); // 500 * 0.2 = 100
}

#[test]
fn test_compute_scroll_threshold_zero() {
    let threshold = compute_scroll_threshold(0.0);
    assert_eq!(threshold, 80.0); // clamped to THRESHOLD_MIN
}

// ── should_load_older ─────────────────────────────────────────────

#[test]
fn test_should_load_older_near_top() {
    assert!(should_load_older(true, 30.0, 80.0));
}

#[test]
fn test_should_load_older_far_from_top() {
    assert!(!should_load_older(true, 200.0, 80.0));
}

#[test]
fn test_should_load_older_cannot_load() {
    assert!(!should_load_older(false, 30.0, 80.0));
}

#[test]
fn test_should_load_older_at_threshold() {
    assert!(!should_load_older(true, 80.0, 80.0));
}

// ── should_load_newer ─────────────────────────────────────────────

#[test]
fn test_should_load_newer_near_bottom() {
    assert!(should_load_newer(true, 500.0, 450.0, 80.0));
}

#[test]
fn test_should_load_newer_far_from_bottom() {
    assert!(!should_load_newer(true, 500.0, 100.0, 80.0));
}

#[test]
fn test_should_load_newer_cannot_load() {
    assert!(!should_load_newer(false, 500.0, 450.0, 80.0));
}

// ── should_show_streaming ─────────────────────────────────────────

#[test]
fn test_should_show_streaming_typical() {
    assert!(should_show_streaming(true, false, "Hello"));
}

#[test]
fn test_should_show_streaming_not_streaming() {
    assert!(!should_show_streaming(false, false, "Hello"));
}

#[test]
fn test_should_show_streaming_has_real_assistant() {
    assert!(!should_show_streaming(true, true, "Hello"));
}

#[test]
fn test_should_show_streaming_empty_content() {
    assert!(!should_show_streaming(true, false, ""));
}

#[test]
fn test_should_show_streaming_whitespace_only() {
    assert!(!should_show_streaming(true, false, "   "));
}

// ── has_real_assistant_message ────────────────────────────────────

#[test]
fn test_has_real_assistant_message_with_assistant() {
    let messages = vec![
        Message { id: "1".into(), role: MessageRole::User, content: "Hi".into(), reasoning_content: None, timestamp: chrono::Utc::now(), status: MessageStatus::Sent },
        Message { id: "2".into(), role: MessageRole::Assistant, content: "Hello".into(), reasoning_content: None, timestamp: chrono::Utc::now(), status: MessageStatus::Sent },
    ];
    assert!(has_real_assistant_message(&messages));
}

#[test]
fn test_has_real_assistant_message_empty_assistant() {
    let messages = vec![
        Message { id: "1".into(), role: MessageRole::User, content: "Hi".into(), reasoning_content: None, timestamp: chrono::Utc::now(), status: MessageStatus::Sent },
        Message { id: "2".into(), role: MessageRole::Assistant, content: String::new(), reasoning_content: None, timestamp: chrono::Utc::now(), status: MessageStatus::Sent },
    ];
    assert!(!has_real_assistant_message(&messages));
}

#[test]
fn test_has_real_assistant_message_only_user() {
    let messages = vec![
        Message { id: "1".into(), role: MessageRole::User, content: "Hi".into(), reasoning_content: None, timestamp: chrono::Utc::now(), status: MessageStatus::Sent },
    ];
    assert!(!has_real_assistant_message(&messages));
}

#[test]
fn test_has_real_assistant_message_empty() {
    let messages: Vec<Message> = vec![];
    assert!(!has_real_assistant_message(&messages));
}
