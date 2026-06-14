use xechat::services::vector_store::lancedb_store::LanceDbStore;

// ── needs_initial_index ───────────────────────────────────────────

#[test]
fn test_needs_initial_index_not_built_enough_rows() {
    assert!(LanceDbStore::needs_initial_index(false, 1000));
}

#[test]
fn test_needs_initial_index_not_built_few_rows() {
    assert!(!LanceDbStore::needs_initial_index(false, 999));
}

#[test]
fn test_needs_initial_index_already_built() {
    assert!(!LanceDbStore::needs_initial_index(true, 2000));
}

// ── needs_rebuild ─────────────────────────────────────────────────

#[test]
fn test_needs_rebuild_not_built() {
    assert!(!LanceDbStore::needs_rebuild(false, 2000, 1000, 0, 100));
}

#[test]
fn test_nebuild_growth_exceeds_threshold() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // 20% growth, well past min interval
    assert!(LanceDbStore::needs_rebuild(true, 1200, 1000, now - 7 * 3600, now));
}

#[test]
fn test_needs_rebuild_force_rebuild() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Past max interval even with no growth
    assert!(LanceDbStore::needs_rebuild(true, 1000, 1000, now - 25 * 3600, now));
}

#[test]
fn test_needs_rebuild_no_rebuild_needed() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // 5% growth, within min interval
    assert!(!LanceDbStore::needs_rebuild(true, 1050, 1000, now - 3600, now));
}

// ── current_timestamp_secs ────────────────────────────────────────

#[test]
fn test_current_timestamp_secs_reasonable() {
    let ts = LanceDbStore::current_timestamp_secs();
    // Should be a reasonable Unix timestamp (after 2020)
    assert!(ts > 1577836800);
}
