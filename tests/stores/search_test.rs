#[path = "../common/mod.rs"]
mod common;

use xechat::stores::search::SearchStore;
use xechat::models::memory::SearchType;

// ── dispatch_search ─────────────────────────────────────────────
// dispatch_search is an async function that depends on database services.
// Without a real database, it should return empty results (via unwrap_or_default).
// These tests verify the function doesn't panic and handles missing DB gracefully.

#[tokio::test]
async fn test_dispatch_search_fulltext_returns_empty_without_db() {
    let _guard = common::setup_temp_dir();
    let results = SearchStore::dispatch_search("test query", &SearchType::FullText).await;
    assert!(results.is_empty(), "FullText search without DB should return empty");
}

#[tokio::test]
async fn test_dispatch_search_semantic_returns_empty_without_db() {
    let _guard = common::setup_temp_dir();
    let results = SearchStore::dispatch_search("test query", &SearchType::Semantic).await;
    assert!(results.is_empty(), "Semantic search without DB should return empty");
}

#[tokio::test]
async fn test_dispatch_search_hybrid_returns_empty_without_db() {
    let _guard = common::setup_temp_dir();
    let results = SearchStore::dispatch_search("test query", &SearchType::Hybrid).await;
    assert!(results.is_empty(), "Hybrid search without DB should return empty");
}
