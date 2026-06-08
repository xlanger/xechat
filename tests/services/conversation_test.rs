#[path = "../common/mod.rs"]
mod common;

use std::collections::HashMap;

use serial_test::serial;
use xechat::Conversation;
use xechat::Message;
use xechat::services::conversation::{
    add_message_to_conversation, create_conversation, delete_conversation,
    load_conversations, rename_conversation, save_conversation,
    update_message_content, write_file,
};
use xechat::services::paths;

fn get_test_id() -> String {
    format!("test-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())
}

#[test]
#[serial]
fn test_create_and_load() {
    let _guard = common::setup_temp_dir();
    let title = format!("Test Chat {}", get_test_id());
    let conv = create_conversation(&title).unwrap();
    assert_eq!(conv.title, title);

    let loaded = load_conversations().unwrap();
    assert!(loaded.iter().any(|c| c.id == conv.id));

    delete_conversation(&conv.id).unwrap();
}

#[test]
#[serial]
fn test_rename_conversation() {
    let _guard = common::setup_temp_dir();
    let id = get_test_id();
    let mut conv = Conversation::new("Original".into());
    conv.id = id.clone();
    save_conversation(&conv).unwrap();

    rename_conversation(&id, "Renamed").unwrap();
    let loaded = load_conversations().unwrap();
    let renamed = loaded.iter().find(|c| c.id == id).unwrap();
    assert_eq!(renamed.title, "Renamed");

    delete_conversation(&id).unwrap();
}

#[test]
#[serial]
fn test_delete_conversation() {
    let _guard = common::setup_temp_dir();
    let id = get_test_id();
    let mut conv = Conversation::new("To Delete".into());
    conv.id = id.clone();
    save_conversation(&conv).unwrap();

    delete_conversation(&id).unwrap();
    let loaded = load_conversations().unwrap();
    assert!(loaded.iter().all(|c| c.id != id));
}

#[test]
#[serial]
fn test_add_message() {
    let _guard = common::setup_temp_dir();
    let id = get_test_id();
    let mut conv = Conversation::new("With Messages".into());
    conv.id = id.clone();
    save_conversation(&conv).unwrap();

    let msg = Message::new_user("Hello".into());
    add_message_to_conversation(&id, &msg).unwrap();

    let loaded = load_conversations().unwrap();
    let conv = loaded.iter().find(|c| c.id == id).unwrap();
    assert_eq!(conv.messages.len(), 1);
    assert_eq!(conv.messages[0].content, "Hello");

    delete_conversation(&id).unwrap();
}

#[test]
#[serial]
fn test_update_message() {
    let _guard = common::setup_temp_dir();
    let id = get_test_id();
    let mut conv = Conversation::new("Update Test".into());
    conv.id = id.clone();
    let msg = Message::new_assistant();
    let msg_id = msg.id.clone();
    conv.messages.push(msg);
    save_conversation(&conv).unwrap();

    update_message_content(&id, &msg_id, "Updated content").unwrap();

    let loaded = load_conversations().unwrap();
    let conv = loaded.iter().find(|c| c.id == id).unwrap();
    assert_eq!(conv.messages[0].content, "Updated content");

    delete_conversation(&id).unwrap();
}

#[test]
#[serial]
fn test_nonexistent_conversation() {
    let _guard = common::setup_temp_dir();
    let result = rename_conversation("nonexistent-id", "New");
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_load_empty_index() {
    let _guard = common::setup_temp_dir();
    let loaded = load_conversations().unwrap();
    assert!(loaded.iter().all(|c| c.id != "nonexistent"));
}

#[test]
#[serial]
fn test_migrate_tauri_data() {
    let _guard = common::setup_temp_dir();

    let mut conv1 = Conversation::new("Old Chat 1".into());
    conv1.id = "old-uuid-1".into();
    conv1.messages.push(Message::new_user("Hello from old".into()));

    let mut conv2 = Conversation::new("Old Chat 2".into());
    conv2.id = "old-uuid-2".into();
    conv2.messages.push(Message::new_user("Second old message".into()));

    let old_index_path = paths::get_conversations_index_path();
    let mut old_data = HashMap::<String, Conversation>::new();
    old_data.insert("old-uuid-1".into(), conv1.clone());
    old_data.insert("old-uuid-2".into(), conv2.clone());
    let json = serde_json::to_string_pretty(&old_data).unwrap();
    write_file(&old_index_path, &json).unwrap();

    let conv1_file = paths::get_conversation_file("old-uuid-1");
    let conv1_json = serde_json::to_string_pretty(&conv1).unwrap();
    write_file(&conv1_file, &conv1_json).unwrap();

    let conv2_file = paths::get_conversation_file("old-uuid-2");
    let conv2_json = serde_json::to_string_pretty(&conv2).unwrap();
    write_file(&conv2_file, &conv2_json).unwrap();

    let loaded = load_conversations().unwrap();
    assert_eq!(loaded.len(), 2);

    let c1 = loaded.iter().find(|c| c.id == "old-uuid-1").unwrap();
    assert_eq!(c1.title, "Old Chat 1");
    assert_eq!(c1.messages.len(), 1);
    assert_eq!(c1.messages[0].content, "Hello from old");

    let c2 = loaded.iter().find(|c| c.id == "old-uuid-2").unwrap();
    assert_eq!(c2.title, "Old Chat 2");
    assert_eq!(c2.messages.len(), 1);

    assert!(conv1_file.exists());
}

#[test]
#[serial]
fn test_migrate_empty_does_nothing() {
    let _guard = common::setup_temp_dir();
    let old_path = paths::get_conversations_index_path();

    write_file(&old_path, "{}").unwrap();

    let loaded = load_conversations().unwrap();
    assert!(loaded.is_empty());

    let content = std::fs::read_to_string(&old_path).unwrap();
    assert_eq!(content.trim(), "{}");
}
