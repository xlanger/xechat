use crate::models::memory::{SearchResult, SearchType};
use std::collections::HashMap;

pub fn reciprocal_rank_fusion(
    fulltext_results: Vec<SearchResult>,
    semantic_results: Vec<SearchResult>,
    k: u32,
) -> Vec<SearchResult> {
    let mut score_map: HashMap<String, f32> = HashMap::new();
    let mut result_map: HashMap<String, SearchResult> = HashMap::new();

    for (rank, result) in fulltext_results.iter().enumerate() {
        // 按 conversation_id 去重，同一对话只保留最高排名的结果
        let key = result.conversation_id.clone();
        let rrf_score = 1.0 / (k as f32 + rank as f32 + 1.0);
        *score_map.entry(key.clone()).or_insert(0.0) += rrf_score;
        result_map.entry(key).or_insert_with(|| result.clone());
    }

    for (rank, result) in semantic_results.iter().enumerate() {
        let key = result.conversation_id.clone();
        let rrf_score = 1.0 / (k as f32 + rank as f32 + 1.0);
        *score_map.entry(key.clone()).or_insert(0.0) += rrf_score;
        result_map.entry(key).or_insert_with(|| result.clone());
    }

    let mut combined: Vec<SearchResult> = result_map
        .into_values()
        .map(|mut r| {
            let key = r.conversation_id.clone();
            r.score = *score_map.get(&key).unwrap_or(&0.0);
            r.search_type = SearchType::Hybrid;
            r
        })
        .collect();

    combined.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    combined
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_result(conv_id: &str, msg_id: &str, score: f32, st: SearchType) -> SearchResult {
        SearchResult {
            conversation_id: conv_id.to_string(),
            message_id: msg_id.to_string(),
            title: String::new(),
            content_snippet: String::new(),
            role: "user".to_string(),
            timestamp: Utc::now(),
            score,
            search_type: st,
            created_at: Utc::now(),
            message_count: 0,
            last_assistant_snippet: String::new(),
        }
    }

    #[test]
    fn test_rrf_basic() {
        let fulltext = vec![
            make_result("c1", "m1", 0.9, SearchType::FullText),
            make_result("c2", "m2", 0.7, SearchType::FullText),
        ];
        let semantic = vec![
            make_result("c2", "m3", 0.8, SearchType::Semantic),
            make_result("c3", "m4", 0.6, SearchType::Semantic),
        ];

        let result = reciprocal_rank_fusion(fulltext, semantic, 60);

        // c1 只在全文中出现，c2 在两者中都出现（RRF 分更高），c3 只在语义中出现
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].search_type, SearchType::Hybrid);

        // c2 应该排在最前（两个来源的 RRF 分叠加）
        assert_eq!(result[0].conversation_id, "c2");
    }

    #[test]
    fn test_rrf_empty() {
        let result = reciprocal_rank_fusion(vec![], vec![], 60);
        assert!(result.is_empty());
    }
}
