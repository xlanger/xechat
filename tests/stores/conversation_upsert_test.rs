use xechat::Conversation;
use xechat::MessageRole;
use xechat::stores::conversation::{upsert_conversation, extract_history_messages};

fn make_conversation(id: &str, messages: Vec<xechat::Message>) -> Conversation {
    Conversation {
        id: id.to_string(),
        title: "Test".to_string(),
        messages,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_temporary: false,
    }
}

fn make_message(id: &str, role: MessageRole, content: &str) -> xechat::Message {
    xechat::Message {
        id: id.to_string(),
        role,
        content: content.to_string(),
        status: xechat::MessageStatus::Sent,
        timestamp: chrono::Utc::now(),
        reasoning_content: None,
    }
}

// ── upsert_conversation ─────────────────────────────────────────

#[test]
fn test_upsert_conversation_insert_new() {
    let mut convs = Vec::new();
    let conv = make_conversation("conv1", vec![]);
    upsert_conversation(&mut convs, "conv1", conv);
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0].id, "conv1");
}

#[test]
fn test_upsert_conversation_replace_existing() {
    let conv1 = make_conversation("conv1", vec![]);
    let mut convs = vec![conv1];
    let conv1_updated = make_conversation("conv1", vec![make_message("m1", MessageRole::User, "hi")]);
    upsert_conversation(&mut convs, "conv1", conv1_updated);
    assert_eq!(convs.len(), 1);
    assert_eq!(convs[0].messages.len(), 1);
}

#[test]
fn test_upsert_conversation_multiple() {
    let mut convs = Vec::new();
    let conv1 = make_conversation("conv1", vec![]);
    let conv2 = make_conversation("conv2", vec![]);
    upsert_conversation(&mut convs, "conv1", conv1);
    upsert_conversation(&mut convs, "conv2", conv2);
    assert_eq!(convs.len(), 2);
}

#[test]
fn test_upsert_conversation_empty_list() {
    let mut convs: Vec<Conversation> = Vec::new();
    let conv = make_conversation("conv1", vec![]);
    upsert_conversation(&mut convs, "conv1", conv);
    assert_eq!(convs.len(), 1);
}

// ── extract_history_messages ────────────────────────────────────

#[test]
fn test_extract_history_messages_empty() {
    let conv = make_conversation("conv1", vec![]);
    let result = extract_history_messages(&conv);
    assert!(result.is_empty());
}

#[test]
fn test_extract_history_messages_user_and_assistant() {
    let conv = make_conversation("conv1", vec![
        make_message("m1", MessageRole::User, "hello"),
        make_message("m2", MessageRole::Assistant, "hi there"),
    ]);
    let result = extract_history_messages(&conv);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].role, "user");
    assert_eq!(result[1].role, "assistant");
}

#[test]
fn test_extract_history_messages_skips_empty_assistant() {
    let conv = make_conversation("conv1", vec![
        make_message("m1", MessageRole::User, "hello"),
        make_message("m2", MessageRole::Assistant, ""),
        make_message("m3", MessageRole::Assistant, "hi there"),
    ]);
    let result = extract_history_messages(&conv);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].role, "user");
    assert_eq!(result[1].role, "assistant");
    assert_eq!(result[1].content, "hi there");
}

#[test]
fn test_extract_history_messages_only_user() {
    let conv = make_conversation("conv1", vec![
        make_message("m1", MessageRole::User, "hello"),
    ]);
    let result = extract_history_messages(&conv);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].role, "user");
}

#[test]
fn test_extract_history_messages_only_empty_assistant() {
    let conv = make_conversation("conv1", vec![
        make_message("m1", MessageRole::Assistant, ""),
    ]);
    let result = extract_history_messages(&conv);
    assert!(result.is_empty());
}
