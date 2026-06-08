use dioxus::prelude::*;
use dioxus_core::{NoOpMutations, Runtime, RuntimeGuard};
use xechat::stores::conversation::{ConversationStore, parse_first_response};
use xechat::Conversation;

fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let mut vdom = VirtualDom::new(|| rsx! { div {} });
    vdom.rebuild(&mut NoOpMutations);
    vdom.in_runtime(|| {
        let runtime = Runtime::current();
        let _guard = RuntimeGuard::new(runtime.clone());
        runtime.in_scope(ScopeId::APP, f)
    })
}

#[test]
fn test_new_default_values() {
    with_runtime(|| {
        let store = ConversationStore::new();

        assert!((store.conversations)().is_empty());
        assert!((store.current_conversation_id)().is_none());
        assert!((store.streaming_content)().is_empty());
        assert!(!(store.is_streaming)());
        assert!(!(store.is_user_scrolling)());
    });
}

#[test]
fn test_selected_conversation_returns_none_when_no_selection() {
    with_runtime(|| {
        let store = ConversationStore::new();

        assert!(store.selected_conversation().is_none());
    });
}

#[test]
fn test_selected_conversation_returns_none_when_id_not_found() {
    with_runtime(|| {
        let mut store = ConversationStore::new();
        store.current_conversation_id.set(Some("nonexistent".to_string()));

        assert!(store.selected_conversation().is_none());
    });
}

#[test]
fn test_selected_conversation_returns_some_when_found() {
    with_runtime(|| {
        let mut store = ConversationStore::new();
        let conv = Conversation::new("Test Chat".to_string());
        let conv_id = conv.id.clone();

        store.conversations.set(vec![conv]);
        store.current_conversation_id.set(Some(conv_id));

        let selected = store.selected_conversation();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().title, "Test Chat");
    });
}

#[test]
fn test_streaming_returns_false_by_default() {
    with_runtime(|| {
        let store = ConversationStore::new();

        assert!(!store.streaming());
    });
}

#[test]
fn test_stream_content_snapshot_returns_empty_by_default() {
    with_runtime(|| {
        let store = ConversationStore::new();

        assert_eq!(store.stream_content_snapshot(), "");
    });
}

#[test]
fn test_select_conversation_sets_current_id() {
    with_runtime(|| {
        let mut store = ConversationStore::new();
        let conv = Conversation::new("Pick Me".to_string());
        let conv_id = conv.id.clone();

        store.conversations.set(vec![conv]);
        assert!((store.current_conversation_id)().is_none());

        store.select_conversation(conv_id.clone());

        assert_eq!((store.current_conversation_id)(), Some(conv_id));
    });
}

#[test]
fn test_select_conversation_clears_streaming_state() {
    with_runtime(|| {
        let mut store = ConversationStore::new();
        let conv = Conversation::new("Chat".to_string());
        let conv_id = conv.id.clone();

        store.conversations.set(vec![conv]);
        store.is_streaming.set(true);
        store.streaming_content.set("partial...".to_string());

        store.select_conversation(conv_id);

        assert!(!(store.is_streaming)());
        assert!((store.streaming_content)().is_empty());
    });
}

#[test]
fn test_select_conversation_removes_temporaries_except_selected() {
    with_runtime(|| {
        let mut store = ConversationStore::new();

        let temp1 = Conversation::new_temporary("Temp 1".to_string());
        let temp2 = Conversation::new_temporary("Temp 2".to_string());
        let permanent = Conversation::new("Permanent".to_string());
        let perm_id = permanent.id.clone();
        let temp1_id = temp1.id.clone();
        let temp2_id = temp2.id.clone();

        store.conversations.set(vec![temp1, temp2, permanent]);

        store.select_conversation(perm_id.clone());

        let convs = (store.conversations)();
        let ids: Vec<&str> = convs.iter().map(|c| c.id.as_str()).collect();

        assert!(ids.contains(&perm_id.as_str()), "permanent conversation should remain");
        assert!(!ids.contains(&temp1_id.as_str()), "temp1 should be removed");
        assert!(!ids.contains(&temp2_id.as_str()), "temp2 should be removed");
        assert_eq!(convs.len(), 1);
    });
}

#[test]
fn test_select_conversation_keeps_selected_temporary() {
    with_runtime(|| {
        let mut store = ConversationStore::new();

        let temp_selected = Conversation::new_temporary("Temp Selected".to_string());
        let temp_other = Conversation::new_temporary("Temp Other".to_string());
        let selected_id = temp_selected.id.clone();
        let other_id = temp_other.id.clone();

        store.conversations.set(vec![temp_selected, temp_other]);

        store.select_conversation(selected_id.clone());

        let convs = (store.conversations)();
        let ids: Vec<&str> = convs.iter().map(|c| c.id.as_str()).collect();

        assert!(ids.contains(&selected_id.as_str()), "selected temporary should remain");
        assert!(!ids.contains(&other_id.as_str()), "other temporary should be removed");
        assert_eq!(convs.len(), 1);
    });
}

#[test]
fn test_select_conversation_preserves_permanent_conversations() {
    with_runtime(|| {
        let mut store = ConversationStore::new();

        let perm1 = Conversation::new("Perm 1".to_string());
        let perm2 = Conversation::new("Perm 2".to_string());
        let perm1_id = perm1.id.clone();
        let perm2_id = perm2.id.clone();

        store.conversations.set(vec![perm1, perm2]);

        store.select_conversation(perm1_id.clone());

        let convs = (store.conversations)();
        let ids: Vec<&str> = convs.iter().map(|c| c.id.as_str()).collect();

        assert!(ids.contains(&perm1_id.as_str()));
        assert!(ids.contains(&perm2_id.as_str()));
        assert_eq!(convs.len(), 2);
    });
}

#[test]
fn test_parse_first_response_with_title() {
    let content = "[TITLE:Rust 所有权机制]\n\nRust 的所有权系统是其最独特的特性...";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, Some("Rust 所有权机制".to_string()));
    assert_eq!(body, "Rust 的所有权系统是其最独特的特性...");
}

#[test]
fn test_parse_first_response_without_title() {
    let content = "这是一个没有标题格式的普通回复。";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, None);
    assert_eq!(body, content);
}

#[test]
fn test_parse_first_response_title_not_at_start() {
    let content = "先有一些内容 [TITLE:不应解析] 后面还有内容。";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, None);
    assert_eq!(body, content);
}

#[test]
fn test_parse_first_response_empty_title() {
    let content = "[TITLE:]\n\n实际回复内容。";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, Some("".to_string()));
    assert_eq!(body, "实际回复内容。");
}

#[test]
fn test_create_temporary_conversation_adds_to_list() {
    with_runtime(|| {
        let mut store = ConversationStore::new();
        assert!((store.conversations)().is_empty());

        let conv_id = store.create_temporary_conversation("New Chat".to_string());

        let convs = (store.conversations)();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].id, conv_id);
        assert_eq!(convs[0].title, "New Chat");
        assert!(convs[0].is_temporary);
    });
}

#[test]
fn test_create_temporary_conversation_sets_current_id() {
    with_runtime(|| {
        let mut store = ConversationStore::new();
        assert!((store.current_conversation_id)().is_none());

        let conv_id = store.create_temporary_conversation("Temp".to_string());

        assert_eq!((store.current_conversation_id)(), Some(conv_id));
    });
}

#[test]
fn test_create_temporary_conversation_inserts_at_head() {
    with_runtime(|| {
        let mut store = ConversationStore::new();
        let existing = Conversation::new("Existing".to_string());
        store.conversations.set(vec![existing]);

        let conv_id = store.create_temporary_conversation("New".to_string());

        let convs = (store.conversations)();
        assert_eq!(convs.len(), 2);
        assert_eq!(convs[0].id, conv_id);
    });
}
