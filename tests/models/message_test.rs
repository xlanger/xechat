use xechat::models::{Message, MessageRole, MessageStatus};

#[test]
fn test_new_user_sets_role_to_user() {
    let msg = Message::new_user("Hello".to_string());
    assert_eq!(msg.role, MessageRole::User);
}

#[test]
fn test_new_user_sets_content() {
    let msg = Message::new_user("Hello world".to_string());
    assert_eq!(msg.content, "Hello world");
}

#[test]
fn test_new_user_sets_status_to_sent() {
    let msg = Message::new_user("Hello".to_string());
    assert_eq!(msg.status, MessageStatus::Sent);
}

#[test]
fn test_new_user_generates_id() {
    let msg = Message::new_user("Hello".to_string());
    assert!(!msg.id.is_empty(), "id should not be empty after creation");
}

#[test]
fn test_new_assistant_sets_role_to_assistant() {
    let msg = Message::new_assistant();
    assert_eq!(msg.role, MessageRole::Assistant);
}

#[test]
fn test_new_assistant_has_empty_content() {
    let msg = Message::new_assistant();
    assert!(msg.content.is_empty(), "new_assistant() should have empty content");
}

#[test]
fn test_new_assistant_sets_status_to_sending() {
    let msg = Message::new_assistant();
    assert_eq!(msg.status, MessageStatus::Sending);
}

#[test]
fn test_new_assistant_generates_id() {
    let msg = Message::new_assistant();
    assert!(!msg.id.is_empty());
}

#[test]
fn test_new_assistant_with_content_sets_content() {
    let msg = Message::new_assistant_with_content("Response text");
    assert_eq!(msg.content, "Response text");
}

#[test]
fn test_new_assistant_with_content_sets_role_to_assistant() {
    let msg = Message::new_assistant_with_content("text");
    assert_eq!(msg.role, MessageRole::Assistant);
}

#[test]
fn test_new_assistant_with_content_sets_status_to_sent() {
    let msg = Message::new_assistant_with_content("text");
    assert_eq!(msg.status, MessageStatus::Sent);
}

#[test]
fn test_new_assistant_with_content_generates_id() {
    let msg = Message::new_assistant_with_content("text");
    assert!(!msg.id.is_empty());
}

#[test]
fn test_message_role_variants() {
    assert_eq!(MessageRole::User, MessageRole::User);
    assert_eq!(MessageRole::Assistant, MessageRole::Assistant);
    assert_ne!(MessageRole::User, MessageRole::Assistant);
}

#[test]
fn test_message_status_variants() {
    assert_eq!(MessageStatus::Sending, MessageStatus::Sending);
    assert_eq!(MessageStatus::Sent, MessageStatus::Sent);
    assert_eq!(MessageStatus::Failed, MessageStatus::Failed);
    assert_ne!(MessageStatus::Sending, MessageStatus::Sent);
    assert_ne!(MessageStatus::Sent, MessageStatus::Failed);
}

#[test]
fn test_user_message_serialization_roundtrip() {
    let msg = Message::new_user("Test content".to_string());
    let json = serde_json::to_string(&msg).expect("serialization should succeed");
    let deserialized: Message = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(msg, deserialized);
}

#[test]
fn test_assistant_message_serialization_roundtrip() {
    let msg = Message::new_assistant_with_content("AI response");
    let json = serde_json::to_string(&msg).expect("serialization should succeed");
    let deserialized: Message = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(msg, deserialized);
}

#[test]
fn test_message_role_serialization() {
    assert_eq!(
        serde_json::to_string(&MessageRole::User).unwrap(),
        serde_json::to_string(&MessageRole::User).unwrap()
    );
    let json = serde_json::to_string(&MessageRole::Assistant).unwrap();
    let role: MessageRole = serde_json::from_str(&json).unwrap();
    assert_eq!(role, MessageRole::Assistant);
}

#[test]
fn test_message_status_serialization() {
    let json = serde_json::to_string(&MessageStatus::Failed).unwrap();
    let status: MessageStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(status, MessageStatus::Failed);
}

#[test]
fn test_each_message_has_unique_id() {
    let msg1 = Message::new_user("First".to_string());
    let msg2 = Message::new_assistant();
    assert_ne!(msg1.id, msg2.id, "each message should have a unique UUID");
}
