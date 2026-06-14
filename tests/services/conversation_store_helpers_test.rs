use xechat::services::conversation_store::ConversationStore;
use xechat::{Message, MessageRole, MessageStatus};

// ── build_updated_message ───────────────────────────────────────

#[test]
fn test_build_updated_message_with_existing_message() {
    let old_msg = Message {
        id: "msg-1".to_string(),
        role: MessageRole::Assistant,
        content: "old content".to_string(),
        reasoning_content: Some("reasoning".to_string()),
        timestamp: chrono::Utc::now(),
        status: MessageStatus::Sent,
    };

    let result = ConversationStore::build_updated_message(Some(&old_msg), "new content", MessageStatus::Truncated);

    let new_msg = result.expect("should return Some");
    assert_eq!(new_msg.id, "msg-1");
    assert_eq!(new_msg.role, MessageRole::Assistant);
    assert_eq!(new_msg.content, "new content");
    assert_eq!(new_msg.reasoning_content, Some("reasoning".to_string()));
    assert_eq!(new_msg.status, MessageStatus::Truncated);
}

#[test]
fn test_build_updated_message_none() {
    let result = ConversationStore::build_updated_message(None, "content", MessageStatus::Sent);
    assert!(result.is_none());
}

#[test]
fn test_build_updated_message_preserves_id() {
    let old_msg = Message {
        id: "original-id".to_string(),
        role: MessageRole::User,
        content: "old".to_string(),
        reasoning_content: None,
        timestamp: chrono::Utc::now(),
        status: MessageStatus::Sent,
    };

    let result = ConversationStore::build_updated_message(Some(&old_msg), "new", MessageStatus::Failed);
    assert_eq!(result.unwrap().id, "original-id");
}

#[test]
fn test_build_updated_message_preserves_role() {
    let old_msg = Message {
        id: "msg-1".to_string(),
        role: MessageRole::User,
        content: "old".to_string(),
        reasoning_content: None,
        timestamp: chrono::Utc::now(),
        status: MessageStatus::Sent,
    };

    let result = ConversationStore::build_updated_message(Some(&old_msg), "new", MessageStatus::Sent);
    assert_eq!(result.unwrap().role, MessageRole::User);
}

#[test]
fn test_build_updated_message_preserves_reasoning_content() {
    let old_msg = Message {
        id: "msg-1".to_string(),
        role: MessageRole::Assistant,
        content: "old".to_string(),
        reasoning_content: Some("step by step".to_string()),
        timestamp: chrono::Utc::now(),
        status: MessageStatus::Sent,
    };

    let result = ConversationStore::build_updated_message(Some(&old_msg), "new", MessageStatus::Sent);
    assert_eq!(result.unwrap().reasoning_content, Some("step by step".to_string()));
}

#[test]
fn test_build_updated_message_no_reasoning_content() {
    let old_msg = Message {
        id: "msg-1".to_string(),
        role: MessageRole::Assistant,
        content: "old".to_string(),
        reasoning_content: None,
        timestamp: chrono::Utc::now(),
        status: MessageStatus::Sent,
    };

    let result = ConversationStore::build_updated_message(Some(&old_msg), "new", MessageStatus::Sent);
    assert_eq!(result.unwrap().reasoning_content, None);
}

#[test]
fn test_build_updated_message_updates_status() {
    let old_msg = Message {
        id: "msg-1".to_string(),
        role: MessageRole::Assistant,
        content: "old".to_string(),
        reasoning_content: None,
        timestamp: chrono::Utc::now(),
        status: MessageStatus::Sending,
    };

    let result = ConversationStore::build_updated_message(Some(&old_msg), "new", MessageStatus::Truncated);
    assert_eq!(result.unwrap().status, MessageStatus::Truncated);
}
