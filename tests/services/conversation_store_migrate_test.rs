use xechat::services::conversation_store::collect_missing_columns;

// ── collect_missing_columns ─────────────────────────────────────

#[test]
fn test_collect_missing_columns_all_present() {
    let existing = vec!["conversation_id".to_string(), "reasoning_content".to_string()];
    let result = collect_missing_columns(&existing);
    assert!(result.is_empty(), "All required columns present, should return empty");
}

#[test]
fn test_collect_missing_columns_missing_one() {
    let existing = vec!["conversation_id".to_string()];
    let result = collect_missing_columns(&existing);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "reasoning_content");
}

#[test]
fn test_collect_missing_columns_empty_existing() {
    let existing: Vec<String> = vec![];
    let result = collect_missing_columns(&existing);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "reasoning_content");
}

#[test]
fn test_collect_missing_columns_unrelated_columns() {
    let existing = vec!["some_other_column".to_string()];
    let result = collect_missing_columns(&existing);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "reasoning_content");
}
