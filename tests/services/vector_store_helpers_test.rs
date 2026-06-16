use arrow_array::{Float32Array, Int32Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema};
use std::sync::Arc;
use xechat::services::vector_store::lancedb_store::LanceDbStore;

// ── needs_initial_index ─────────────────────────────────────────────

#[test]
fn test_needs_initial_index_not_built_below_threshold() {
    assert!(!LanceDbStore::needs_initial_index(false, 0));
    assert!(!LanceDbStore::needs_initial_index(false, 500));
    assert!(!LanceDbStore::needs_initial_index(false, 9999));
}

#[test]
fn test_needs_initial_index_not_built_at_threshold() {
    assert!(LanceDbStore::needs_initial_index(false, 10000));
}

#[test]
fn test_needs_initial_index_not_built_above_threshold() {
    assert!(LanceDbStore::needs_initial_index(false, 20000));
}

#[test]
fn test_needs_initial_index_already_built() {
    assert!(!LanceDbStore::needs_initial_index(true, 10000));
    assert!(!LanceDbStore::needs_initial_index(true, 50000));
}

// ── needs_rebuild ───────────────────────────────────────────────────

#[test]
fn test_needs_rebuild_not_built() {
    // If index was never built, should not rebuild (needs_initial_index handles that)
    assert!(!LanceDbStore::needs_rebuild(false, 20000, 10000, 0, 100));
}

#[test]
fn test_needs_rebuild_no_growth() {
    let now = 1000000;
    let last_time = now - 100; // within min interval
    assert!(!LanceDbStore::needs_rebuild(true, 10000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_significant_growth_within_min_interval() {
    let now = 1000000;
    let last_time = now - 100; // less than VECTOR_INDEX_REBUILD_MIN_SECS
    // Growth: (12000 - 10000) * 100 / 10000 = 20% >= 10%
    assert!(!LanceDbStore::needs_rebuild(true, 12000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_significant_growth_after_min_interval() {
    let now = 1000000;
    let last_time = now - 7 * 3600; // > VECTOR_INDEX_REBUILD_MIN_SECS (6 hours)
    // Growth: (12000 - 10000) * 100 / 10000 = 20% >= 10%
    assert!(LanceDbStore::needs_rebuild(true, 12000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_force_after_max_interval() {
    let now = 1000000;
    let last_time = now - 25 * 3600; // > VECTOR_INDEX_REBUILD_MAX_SECS (24 hours)
    // Even with no growth
    assert!(LanceDbStore::needs_rebuild(true, 10000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_small_growth_after_min_interval() {
    let now = 1000000;
    let last_time = now - 7 * 3600;
    // Growth: (10500 - 10000) * 100 / 10000 = 5% < 10%
    assert!(!LanceDbStore::needs_rebuild(true, 10500, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_zero_last_rows() {
    let now = 1000000;
    let last_time = now - 7 * 3600;
    // When last_rows is 0, growth is 100%, which is >= 10%
    assert!(LanceDbStore::needs_rebuild(true, 100, 0, last_time, now));
}

// ── batch_to_hits ───────────────────────────────────────────────────

fn make_hits_batch(
    conv_ids: Vec<String>,
    asst_contents: Vec<String>,
    distances: Vec<f32>,
) -> RecordBatch {
    let n = conv_ids.len();
    let schema = Arc::new(Schema::new(vec![
        Field::new("conversation_id", DataType::Utf8, false),
        Field::new("user_message_id", DataType::Utf8, false),
        Field::new("assistant_message_id", DataType::Utf8, false),
        Field::new("user_content", DataType::Utf8, false),
        Field::new("assistant_content", DataType::Utf8, false),
        Field::new("chunk_index", DataType::Int32, false),
        Field::new("timestamp", DataType::Utf8, false),
        Field::new("_distance", DataType::Float32, false),
    ]));

    let user_msg_ids: Vec<String> = std::iter::repeat(String::new()).take(n).collect();
    let asst_msg_ids: Vec<String> = (0..n).map(|i| format!("msg_{}", i)).collect();
    let user_contents: Vec<String> = std::iter::repeat(String::new()).take(n).collect();
    let chunk_indices: Vec<i32> = std::iter::repeat(0).take(n).collect();
    let timestamps: Vec<String> = std::iter::repeat("2024-01-01T00:00:00Z".to_string()).take(n).collect();

    RecordBatch::try_new(schema, vec![
        Arc::new(StringArray::from(conv_ids)),
        Arc::new(StringArray::from(user_msg_ids)),
        Arc::new(StringArray::from(asst_msg_ids)),
        Arc::new(StringArray::from(user_contents)),
        Arc::new(StringArray::from(asst_contents)),
        Arc::new(Int32Array::from(chunk_indices)),
        Arc::new(StringArray::from(timestamps)),
        Arc::new(Float32Array::from(distances)),
    ]).unwrap()
}

#[test]
fn test_batch_to_hits_basic() {
    let batch = make_hits_batch(
        vec!["conv1".to_string()],
        vec!["Hello world".to_string()],
        vec![0.1],
    );
    let hits = LanceDbStore::batch_to_hits(&batch);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].conversation_id, "conv1");
    assert_eq!(hits[0].content, "Hello world");
    assert!((hits[0].score - 0.9).abs() < 0.001); // score = 1.0 - distance
}

#[test]
fn test_batch_to_hits_multiple_rows() {
    let batch = make_hits_batch(
        vec!["conv1".to_string(), "conv2".to_string()],
        vec!["content1".to_string(), "content2".to_string()],
        vec![0.2, 0.5],
    );
    let hits = LanceDbStore::batch_to_hits(&batch);

    assert_eq!(hits.len(), 2);
    assert!((hits[0].score - 0.8).abs() < 0.001);
    assert!((hits[1].score - 0.5).abs() < 0.001);
}

#[test]
fn test_batch_to_hits_missing_distance_returns_empty() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("assistant_content", DataType::Utf8, false),
    ]));
    let batch = RecordBatch::try_new(schema, vec![
        Arc::new(StringArray::from(vec!["content"])),
    ]).unwrap();

    let hits = LanceDbStore::batch_to_hits(&batch);
    assert!(hits.is_empty());
}

#[test]
fn test_batch_to_hits_empty_batch() {
    let batch = make_hits_batch(
        Vec::<String>::new(),
        Vec::<String>::new(),
        Vec::<f32>::new(),
    );
    let hits = LanceDbStore::batch_to_hits(&batch);
    assert!(hits.is_empty());
}
