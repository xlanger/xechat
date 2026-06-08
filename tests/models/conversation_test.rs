use xechat::models::Conversation;

#[test]
fn test_new_creates_conversation_with_id() {
    let conv = Conversation::new("Test Chat".to_string());
    assert!(!conv.id.is_empty(), "id should not be empty after creation");
}

#[test]
fn test_new_creates_conversation_with_given_title() {
    let conv = Conversation::new("My Title".to_string());
    assert_eq!(conv.title, "My Title");
}

#[test]
fn test_new_creates_conversation_with_empty_messages() {
    let conv = Conversation::new("Test".to_string());
    assert!(conv.messages.is_empty(), "messages should be empty after creation");
}

#[test]
fn test_new_creates_non_temporary_conversation() {
    let conv = Conversation::new("Test".to_string());
    assert!(!conv.is_temporary, "new() should create a non-temporary conversation");
}

#[test]
fn test_new_sets_created_and_updated_at() {
    let conv = Conversation::new("Test".to_string());
    assert_eq!(conv.created_at, conv.updated_at, "created_at and updated_at should be equal on creation");
}

#[test]
fn test_new_temporary_sets_is_temporary_flag() {
    let conv = Conversation::new_temporary("Temp Chat".to_string());
    assert!(conv.is_temporary, "new_temporary() should set is_temporary to true");
}

#[test]
fn test_new_temporary_has_same_fields_as_new() {
    let conv = Conversation::new_temporary("Temp".to_string());
    assert!(!conv.id.is_empty());
    assert_eq!(conv.title, "Temp");
    assert!(conv.messages.is_empty());
    assert_eq!(conv.created_at, conv.updated_at);
}

#[test]
fn test_new_vs_new_temporary_difference() {
    let normal = Conversation::new("A".to_string());
    let temp = Conversation::new_temporary("A".to_string());
    assert!(!normal.is_temporary);
    assert!(temp.is_temporary);
}

#[test]
fn test_serialization_roundtrip_preserves_fields() {
    let conv = Conversation::new("Serialize Test".to_string());
    let json = serde_json::to_string(&conv).expect("serialization should succeed");
    let deserialized: Conversation = serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(conv.id, deserialized.id);
    assert_eq!(conv.title, deserialized.title);
    assert_eq!(conv.messages, deserialized.messages);
    assert_eq!(conv.created_at, deserialized.created_at);
    assert_eq!(conv.updated_at, deserialized.updated_at);
}

#[test]
fn test_is_temporary_skipped_in_serialization() {
    let conv = Conversation::new_temporary("Temp".to_string());
    assert!(conv.is_temporary);
    let json = serde_json::to_string(&conv).expect("serialization should succeed");
    let deserialized: Conversation = serde_json::from_str(&json).expect("deserialization should succeed");
    assert!(!deserialized.is_temporary, "is_temporary should be skipped (serde skip) and default to false");
}

#[test]
fn test_each_new_conversation_has_unique_id() {
    let conv1 = Conversation::new("First".to_string());
    let conv2 = Conversation::new("Second".to_string());
    assert_ne!(conv1.id, conv2.id, "each conversation should have a unique UUID");
}
