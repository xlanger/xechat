pub mod hybrid;

use crate::models::memory::SearchResult;

pub async fn fulltext_search(query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
    let store = crate::services::conversation_store::get_store()
        .ok_or_else(|| anyhow::anyhow!("ConversationStore not initialized"))?;
    store.search_fulltext(query, limit).await
}

pub async fn semantic_search(query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
    let embedder = crate::services::embedder::get_embedder()
        .ok_or_else(|| anyhow::anyhow!("Embedder not initialized"))?;

    eprintln!("[xechat:search] query='{}' embedder={} dim={}", query, embedder.name(), embedder.dimension());

    let query_vector = embedder.encode_query(query).await?;

    let store = crate::services::conversation_store::get_store()
        .ok_or_else(|| anyhow::anyhow!("ConversationStore not initialized"))?;

    let result = store.search_semantic(&query_vector, limit).await;

    eprintln!("[xechat:search] result count={}", result.as_ref().map(|r| r.len()).unwrap_or(0));

    result
}
