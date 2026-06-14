use xechat::services::conversation_store::collect_missing_columns;

// ── collect_missing_columns ───────────────────────────────────────

#[test]
fn test_collect_missing_columns_all_present() {
    let existing = vec!["conversation_id".to_string(), "reasoning_content".to_string()];
    let missing = collect_missing_columns(&existing);
    assert!(missing.is_empty());
}

#[test]
fn test_collect_missing_columns_missing_reasoning() {
    let existing = vec!["conversation_id".to_string()];
    let missing = collect_missing_columns(&existing);
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].0, "reasoning_content");
    assert_eq!(missing[0].1, "''");
}

#[test]
fn test_collect_missing_columns_empty_existing() {
    let existing: Vec<String> = vec![];
    let missing = collect_missing_columns(&existing);
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].0, "reasoning_content");
}
