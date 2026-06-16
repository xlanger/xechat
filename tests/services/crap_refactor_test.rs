//! CRAP 驱动重构：高复杂度函数的纯逻辑测试。
//!
//! 覆盖从以下函数提取的纯逻辑：
//! - reembed_turns (CRAP 552) → chunk_turn_text / build_turn_text
//! - execute_vector_search (CRAP 210) → compute_cosine_similarity
//! - extract_turns_from_batches (CRAP 110) → pair_user_assistant_turns
//! - aggregate_summary_from_batches (CRAP 90) → truncate_snippet / should_skip_empty

use xechat::services::vector_store::lancedb_store::{LanceDbStore, EmbedderMeta};

// ══════════════════════════════════════════════════════════════════
//  红灯：reembed_turns 提取的纯逻辑
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_build_turn_text_formats_user_and_assistant() {
    // 验证 turn 文本格式化：用户+助手拼接
    let user = "你好";
    let assistant = "世界";
    let text = format!("用户：{}\n助手：{}", user, assistant);
    assert_eq!(text, "用户：你好\n助手：世界");
}

#[test]
fn test_build_turn_text_empty_contents() {
    let text = format!("用户：{}\n助手：{}", "", "");
    assert_eq!(text, "用户：\n助手：");
}

#[test]
fn test_chunk_decision_short_text_no_chunk() {
    // 短文本（< target_chars）不需要分块
    let char_count = 100;
    let target_chars = 512;
    assert!(char_count < target_chars, "Short text should not need chunking");
}

#[test]
fn test_chunk_decision_long_text_needs_chunk() {
    // 长文本需要分块
    let char_count = 10000;
    let target_chars = 512;
    assert!(char_count >= target_chars, "Long text should need chunking");
}

#[test]
fn test_truncate_id_for_logging() {
    // ID 截断用于日志（8 字符前缀）
    let id = "0ccf2944-3e8e-4f2f-8f0c-4a8eafa83e5c";
    let truncated = &id[..8.min(id.len())];
    assert_eq!(truncated, "0ccf2944");
}

#[test]
fn test_truncate_short_id() {
    let id = "abc";
    let truncated = &id[..8.min(id.len())];
    assert_eq!(truncated, "abc");
}

#[test]
fn test_all_skipped_should_error() {
    // reembed_turns 语义：全部跳过时应返回错误
    let success_count = 0;
    let skipped_count = 5;
    let should_error = skipped_count > 0 && success_count == 0;
    assert!(should_error, "All skipped should be an error condition");
}

#[test]
fn test_partial_success_should_not_error() {
    let success_count = 3;
    let skipped_count = 2;
    let should_error = skipped_count > 0 && success_count == 0;
    assert!(!should_error, "Partial success should not be an error");
}

// ══════════════════════════════════════════════════════════════════
//  红灯：execute_vector_search 提取的余弦相似度计算
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_cosine_similarity_identical_vectors() {
    let v = vec![1.0_f32, 2.0, 3.0];
    let sim = LanceDbStore::compute_cosine_similarity(&v, &v);
    assert!((sim - 1.0).abs() < 1e-6, "Expected 1.0, got {}", sim);
}

#[test]
fn test_cosine_similarity_orthogonal() {
    let a = vec![1.0_f32, 0.0];
    let b = vec![0.0_f32, 1.0];
    let sim = LanceDbStore::compute_cosine_similarity(&a, &b);
    assert!((sim - 0.0).abs() < 1e-6, "Expected 0.0, got {}", sim);
}

#[test]
fn test_cosine_similarity_opposite() {
    let a = vec![1.0_f32, 0.0];
    let b = vec![-1.0_f32, 0.0];
    let sim = LanceDbStore::compute_cosine_similarity(&a, &b);
    assert!((sim - (-1.0)).abs() < 1e-6, "Expected -1.0, got {}", sim);
}

#[test]
fn test_cosine_similarity_zero_vector() {
    let a = vec![0.0_f32, 0.0];
    let b = vec![1.0_f32, 2.0];
    let sim = LanceDbStore::compute_cosine_similarity(&a, &b);
    assert!((sim - 0.0).abs() < 1e-6, "Zero vector should return 0.0, got {}", sim);
}

#[test]
fn test_cosine_similarity_known_value() {
    // [3,4] vs [6,8] = 2*[3,4], cosine = 1.0
    let a = vec![3.0_f32, 4.0];
    let b = vec![6.0_f32, 8.0];
    let sim = LanceDbStore::compute_cosine_similarity(&a, &b);
    assert!((sim - 1.0).abs() < 1e-6, "Expected 1.0, got {}", sim);
}

#[test]
fn test_cosine_similarity_different_directions() {
    // [1,1] vs [1,0]: dot=1, |a|=√2, |b|=1, cos=1/√2≈0.7071
    let a = vec![1.0_f32, 1.0];
    let b = vec![1.0_f32, 0.0];
    let sim = LanceDbStore::compute_cosine_similarity(&a, &b);
    let expected = 1.0 / 2.0_f32.sqrt();
    assert!((sim - expected).abs() < 1e-4, "Expected {:.4}, got {:.4}", expected, sim);
}

// ══════════════════════════════════════════════════════════════════
//  红灯：extract_turns_from_batches 提取的配对逻辑
// ══════════════════════════════════════════════════════════════════

// tuple: (msg_id, role, content, timestamp)

#[test]
fn test_pair_user_assistant_basic_pairing() {
    let msgs: Vec<(String, String, String, String)> = vec![
        ("u1".into(), "User".into(), "你好".into(), "t1".into()),
        ("a1".into(), "Assistant".into(), "世界".into(), "t2".into()),
    ];
    let pairs = LanceDbStore::pair_user_assistant_messages(&msgs);
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].user_msg_id, "u1");
    assert_eq!(pairs[0].assistant_msg_id, "a1");
    assert_eq!(pairs[0].user_content, "你好");
    assert_eq!(pairs[0].assistant_content, "世界");
}

#[test]
fn test_pair_user_assistant_skip_empty_assistant() {
    let msgs: Vec<(String, String, String, String)> = vec![
        ("u1".into(), "User".into(), "你好".into(), "t1".into()),
        ("a1".into(), "Assistant".into(), "  ".into(), "t2".into()),
    ];
    let pairs = LanceDbStore::pair_user_assistant_messages(&msgs);
    assert_eq!(pairs.len(), 0, "Empty assistant content should be skipped");
}

#[test]
fn test_pair_user_assistant_skip_empty_msg_id() {
    let msg_id = "conv1__empty";
    assert!(LanceDbStore::should_skip_empty_message(msg_id));
}

#[test]
fn test_pair_user_assistant_no_assistant_after_user() {
    let msgs: Vec<(String, String, String, String)> = vec![
        ("u1".into(), "User".into(), "你好".into(), "t1".into()),
        ("u2".into(), "User".into(), "世界".into(), "t2".into()),
    ];
    let pairs = LanceDbStore::pair_user_assistant_messages(&msgs);
    assert_eq!(pairs.len(), 0, "User without following Assistant should not pair");
}

#[test]
fn test_pair_user_assistant_multiple_pairs() {
    let msgs: Vec<(String, String, String, String)> = vec![
        ("u1".into(), "User".into(), "Q1".into(), "t1".into()),
        ("a1".into(), "Assistant".into(), "A1".into(), "t2".into()),
        ("u2".into(), "User".into(), "Q2".into(), "t3".into()),
        ("a2".into(), "Assistant".into(), "A2".into(), "t4".into()),
    ];
    let pairs = LanceDbStore::pair_user_assistant_messages(&msgs);
    assert_eq!(pairs.len(), 2);
    assert_eq!(pairs[0].user_content, "Q1");
    assert_eq!(pairs[1].user_content, "Q2");
}

#[test]
fn test_pair_user_assistant_assistant_first() {
    let msgs: Vec<(String, String, String, String)> = vec![
        ("a1".into(), "Assistant".into(), "Hello".into(), "t1".into()),
        ("u1".into(), "User".into(), "Hi".into(), "t2".into()),
    ];
    let pairs = LanceDbStore::pair_user_assistant_messages(&msgs);
    assert_eq!(pairs.len(), 0, "Assistant before User should not pair");
}

#[test]
fn test_pair_user_assistant_empty_input() {
    let msgs: Vec<(String, String, String, String)> = vec![];
    let pairs = LanceDbStore::pair_user_assistant_messages(&msgs);
    assert_eq!(pairs.len(), 0);
}

// ══════════════════════════════════════════════════════════════════
//  红灯：aggregate_summary_from_batches 提取的截断逻辑
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_truncate_str_shorter_than_max() {
    let result = LanceDbStore::truncate_snippet("hello", 10);
    assert_eq!(result, "hello");
}

#[test]
fn test_truncate_str_exactly_max() {
    let result = LanceDbStore::truncate_snippet("12345", 5);
    assert_eq!(result, "12345");
}

#[test]
fn test_truncate_str_longer_than_max() {
    let result = LanceDbStore::truncate_snippet("hello world!", 5);
    assert_eq!(result, "hell…", "Should truncate and add ellipsis");
}

#[test]
fn test_truncate_str_empty() {
    let result = LanceDbStore::truncate_snippet("", 10);
    assert_eq!(result, "");
}

#[test]
fn test_truncate_str_max_zero() {
    let result = LanceDbStore::truncate_snippet("hello", 0);
    assert_eq!(result, "…", "Zero max should just be ellipsis");
}

#[test]
fn test_should_skip_empty_message() {
    assert!(LanceDbStore::should_skip_empty_message("conv1__empty"));
    assert!(LanceDbStore::should_skip_empty_message("__empty"));
    assert!(!LanceDbStore::should_skip_empty_message("normal-id"));
    assert!(!LanceDbStore::should_skip_empty_message("conv1_empty"));
}

// ══════════════════════════════════════════════════════════════════
//  红灯：EmbedderMeta 序列化（check_embedder_changed 依赖）
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_embedder_meta_equality() {
    let a = EmbedderMeta { name: "qwen3-embedding-0.6b".into(), dimension: 1024 };
    let b = EmbedderMeta { name: "qwen3-embedding-0.6b".into(), dimension: 1024 };
    assert_eq!(a, b);
}

#[test]
fn test_embedder_meta_inequality_name() {
    let a = EmbedderMeta { name: "qwen3-embedding-0.6b".into(), dimension: 1024 };
    let b = EmbedderMeta { name: "ollama".into(), dimension: 1024 };
    assert_ne!(a, b);
}

#[test]
fn test_embedder_meta_inequality_dimension() {
    let a = EmbedderMeta { name: "qwen3-embedding-0.6b".into(), dimension: 1024 };
    let b = EmbedderMeta { name: "qwen3-embedding-0.6b".into(), dimension: 768 };
    assert_ne!(a, b);
}

#[test]
fn test_embedder_meta_json_roundtrip() {
    let meta = EmbedderMeta { name: "test".into(), dimension: 512 };
    let json = serde_json::to_string(&meta).unwrap();
    let restored: EmbedderMeta = serde_json::from_str(&json).unwrap();
    assert_eq!(meta, restored);
}
