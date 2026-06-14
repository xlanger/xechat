use xechat::services::embedder::manager::compute_next_chunk_start;

// ── compute_next_chunk_start ──────────────────────────────────────

#[test]
fn test_compute_next_chunk_start_normal() {
    let text = "Hello world, this is a test of chunk overlap computation.";
    let end = 20;
    let overlap = 5;
    let result = compute_next_chunk_start(text, end, overlap);
    assert!(result < end);
    assert!(result > 0);
}

#[test]
fn test_compute_next_chunk_start_overlap_exceeds_end() {
    let text = "Short";
    let end = 3;
    let overlap = 10;
    let result = compute_next_chunk_start(text, end, overlap);
    assert_eq!(result, 0);
}

#[test]
fn test_compute_next_chunk_start_zero_overlap() {
    // When overlap is 0, char_offset_back returns 0 (no backward offset),
    // so the next chunk starts at position 0 (beginning of text).
    let text = "Hello world";
    let end = 5;
    let overlap = 0;
    let result = compute_next_chunk_start(text, end, overlap);
    assert_eq!(result, 0);
}

#[test]
fn test_compute_next_chunk_start_exact_overlap() {
    let text = "Hello world";
    let end = 5;
    let overlap = 5;
    let result = compute_next_chunk_start(text, end, overlap);
    assert_eq!(result, 0);
}
