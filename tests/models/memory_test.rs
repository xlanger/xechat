use xechat::models::memory::{SearchHit, TurnEntry, ChunkMeta, IntentResult, IntentAction, TimeRange};
use chrono::Utc;

#[test]
fn test_turn_entry_serialization() {
    let entry = TurnEntry {
        id: "turn-1".to_string(),
        conversation_id: "conv-id".to_string(),
        user_message_id: "msg-user-1".to_string(),
        assistant_message_id: "msg-asst-1".to_string(),
        turn_index: 0,
        user_content: "你好".to_string(),
        assistant_content: "你好！".to_string(),
        timestamp: Utc::now(),
        chunks: vec![ChunkMeta {
            chunk_index: 0,
            chunk_text: "用户：你好\n助手：你好！".to_string(),
            start_char: 0,
            end_char: 14,
            embedding: vec![0.1; 768],
        }],
    };
    let json = serde_json::to_string(&entry).unwrap();
    let deserialized: TurnEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "turn-1");
    assert_eq!(deserialized.conversation_id, "conv-id");
    // embedding is skipped by serde
    assert!(deserialized.chunks[0].embedding.is_empty());
}

#[test]
fn test_intent_result_default_action() {
    let result = IntentResult {
        needs_memory: false,
        confidence: 0.9,
        memory_query: "test".to_string(),
        time_hint: TimeRange::Any,
        action: IntentAction::DirectQuery,
    };
    assert!(!result.needs_memory);
    assert_eq!(result.action, IntentAction::DirectQuery);
}

#[test]
fn test_search_hit_ordering() {
    let hit1 = SearchHit {
        score: 0.8,
        entry_id: "1".to_string(),
        conversation_id: "c1".to_string(),
        message_id: "m1".to_string(),
        content: "hello".to_string(),
        role: "user".to_string(),
        timestamp: "2026-01-01T00:00:00Z".to_string(),
        user_message_id: String::new(),
        user_content: String::new(),
        chunk_index: -1,
    };
    let hit2 = SearchHit {
        score: 0.9,
        entry_id: "2".to_string(),
        conversation_id: "c2".to_string(),
        message_id: "m2".to_string(),
        content: "world".to_string(),
        role: "assistant".to_string(),
        timestamp: "2026-01-01T00:00:01Z".to_string(),
        user_message_id: String::new(),
        user_content: String::new(),
        chunk_index: -1,
    };
    assert!(hit2.score > hit1.score);
}
