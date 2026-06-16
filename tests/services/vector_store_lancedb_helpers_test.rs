use xechat::services::vector_store::lancedb_store::LanceDbStore;

// ── current_timestamp_secs ──────────────────────────────────────

#[test]
fn test_current_timestamp_secs_returns_reasonable_value() {
    let ts = LanceDbStore::current_timestamp_secs();
    // Should be a reasonable Unix timestamp (after year 2020 = 1577836800)
    assert!(ts > 1577836800, "Timestamp should be after 2020, got {}", ts);
    // Should be less than year 2100 = 4102444800
    assert!(ts < 4102444800, "Timestamp should be before 2100, got {}", ts);
}

#[test]
fn test_current_timestamp_secs_monotonic() {
    let ts1 = LanceDbStore::current_timestamp_secs();
    let ts2 = LanceDbStore::current_timestamp_secs();
    assert!(ts2 >= ts1, "Timestamps should be monotonically increasing");
}

// ── needs_initial_index ─────────────────────────────────────────

#[test]
fn test_needs_initial_index_not_built_enough_rows() {
    assert!(LanceDbStore::needs_initial_index(false, 10000));
}

#[test]
fn test_needs_initial_index_not_built_few_rows() {
    assert!(!LanceDbStore::needs_initial_index(false, 9999));
}

#[test]
fn test_needs_initial_index_already_built() {
    assert!(!LanceDbStore::needs_initial_index(true, 50000));
}

#[test]
fn test_needs_initial_index_already_built_zero_rows() {
    assert!(!LanceDbStore::needs_initial_index(true, 0));
}

#[test]
fn test_needs_initial_index_not_built_zero_rows() {
    assert!(!LanceDbStore::needs_initial_index(false, 0));
}

// ── needs_rebuild ───────────────────────────────────────────────

#[test]
fn test_needs_rebuild_not_built() {
    assert!(!LanceDbStore::needs_rebuild(false, 20000, 10000, 0, 1000000));
}

#[test]
fn test_needs_rebuild_growth_above_threshold_and_min_time_elapsed() {
    // count=11000, last_rows=10000 => 10% growth, elapsed >= 6h
    let now: u64 = 1000000;
    let last_time = now - 7 * 3600; // 7 hours ago
    assert!(LanceDbStore::needs_rebuild(true, 11000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_growth_above_threshold_but_min_time_not_elapsed() {
    // count=11000, last_rows=10000 => 10% growth, but elapsed < 6h
    let now: u64 = 1000000;
    let last_time = now - 3600; // 1 hour ago
    assert!(!LanceDbStore::needs_rebuild(true, 11000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_growth_below_threshold() {
    // count=10500, last_rows=10000 => 5% growth
    let now: u64 = 1000000;
    let last_time = now - 7 * 3600;
    assert!(!LanceDbStore::needs_rebuild(true, 10500, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_force_rebuild_after_max_time() {
    // Even with 0% growth, force rebuild after 24h
    let now: u64 = 1000000;
    let last_time = now - 25 * 3600; // 25 hours ago
    assert!(LanceDbStore::needs_rebuild(true, 10000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_no_force_rebuild_within_max_time() {
    // No growth, within 24h
    let now: u64 = 1000000;
    let last_time = now - 12 * 3600; // 12 hours ago
    assert!(!LanceDbStore::needs_rebuild(true, 10000, 10000, last_time, now));
}

#[test]
fn test_needs_rebuild_zero_last_rows() {
    // last_rows=0 => growth_pct=100%, which is above threshold
    let now: u64 = 1000000;
    let last_time = now - 7 * 3600;
    assert!(LanceDbStore::needs_rebuild(true, 500, 0, last_time, now));
}
