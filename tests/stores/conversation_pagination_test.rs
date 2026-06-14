use xechat::stores::conversation::{
    MessagePagination,
    compute_full_window_range, compute_anchored_window_range,
    can_load_older, can_load_newer, compute_older_window, compute_newer_window_end,
    sync_ollama_host_to_provider,
};
use xechat::models::config::{XEChatConfig, ModelProvider};

fn make_model_provider(base_url: &str) -> ModelProvider {
    ModelProvider {
        name: "test".to_string(),
        api_key: String::new(),
        base_url: base_url.to_string(),
        timeout: None,
        models: std::collections::HashMap::new(),
    }
}

// ── compute_full_window_range ─────────────────────────────────────

#[test]
fn test_compute_full_window_range_messages_less_than_page() {
    let (start, end) = compute_full_window_range(5, 10);
    assert_eq!(start, 0);
    assert_eq!(end, 5);
}

#[test]
fn test_compute_full_window_range_messages_equal_to_page() {
    let (start, end) = compute_full_window_range(10, 10);
    assert_eq!(start, 0);
    assert_eq!(end, 10);
}

#[test]
fn test_compute_full_window_range_messages_more_than_page() {
    let (start, end) = compute_full_window_range(25, 10);
    assert_eq!(start, 15);
    assert_eq!(end, 25);
}

#[test]
fn test_compute_full_window_range_zero_messages() {
    let (start, end) = compute_full_window_range(0, 10);
    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

// ── compute_anchored_window_range ─────────────────────────────────

#[test]
fn test_compute_anchored_window_range_start() {
    let (start, end) = compute_anchored_window_range(20, 10, 0);
    assert_eq!(start, 0);
    assert_eq!(end, 10);
}

#[test]
fn test_compute_anchored_window_range_middle() {
    let (start, end) = compute_anchored_window_range(20, 10, 5);
    assert_eq!(start, 5);
    assert_eq!(end, 15);
}

#[test]
fn test_compute_anchored_window_range_near_end() {
    let (start, end) = compute_anchored_window_range(20, 10, 18);
    assert_eq!(start, 18);
    assert_eq!(end, 20);
}

#[test]
fn test_compute_anchored_window_range_at_end() {
    let (start, end) = compute_anchored_window_range(20, 10, 20);
    assert_eq!(start, 20);
    assert_eq!(end, 20);
}

// ── can_load_older ────────────────────────────────────────────────

#[test]
fn test_can_load_older_loading() {
    let pg = MessagePagination {
        all_messages: vec![],
        start_index: 5,
        end_index: 10,
        page_size: 10,
        is_loading: true,
    };
    assert!(can_load_older(&pg).is_none());
}

#[test]
fn test_can_load_older_at_top() {
    let pg = MessagePagination {
        all_messages: vec![],
        start_index: 0,
        end_index: 10,
        page_size: 10,
        is_loading: false,
    };
    assert!(can_load_older(&pg).is_none());
}

#[test]
fn test_can_load_older_can_load() {
    let pg = MessagePagination {
        all_messages: vec![],
        start_index: 5,
        end_index: 15,
        page_size: 10,
        is_loading: false,
    };
    let result = can_load_older(&pg);
    assert!(result.is_some());
    let (start, page_size, _all_len) = result.unwrap();
    assert_eq!(start, 5);
    assert_eq!(page_size, 10);
}

// ── can_load_newer ────────────────────────────────────────────────

#[test]
fn test_can_load_newer_loading() {
    let pg = MessagePagination {
        all_messages: vec![],
        start_index: 0,
        end_index: 10,
        page_size: 10,
        is_loading: true,
    };
    assert!(can_load_newer(&pg).is_none());
}

#[test]
fn test_can_load_newer_at_bottom() {
    let pg = MessagePagination {
        all_messages: vec![],
        start_index: 0,
        end_index: 10,
        page_size: 10,
        is_loading: false,
    };
    assert!(can_load_newer(&pg).is_none());
}

#[test]
fn test_can_load_newer_can_load() {
    use xechat::Message;
    let pg = MessagePagination {
        // Need all_messages.len() > end_index for can_load_newer to return Some
        all_messages: vec![Message::new_user("hi".to_string()); 20],
        start_index: 0,
        end_index: 5,
        page_size: 10,
        is_loading: false,
    };
    let result = can_load_newer(&pg);
    assert!(result.is_some());
    let (end, page_size, _all_len) = result.unwrap();
    assert_eq!(end, 5);
    assert_eq!(page_size, 10);
}

// ── compute_older_window ──────────────────────────────────────────

#[test]
fn test_compute_older_window_normal() {
    assert_eq!(compute_older_window(15, 10), 5);
}

#[test]
fn test_compute_older_window_saturating() {
    assert_eq!(compute_older_window(5, 10), 0);
}

#[test]
fn test_compute_older_window_exact() {
    assert_eq!(compute_older_window(10, 10), 0);
}

// ── compute_newer_window_end ──────────────────────────────────────

#[test]
fn test_compute_newer_window_end_normal() {
    assert_eq!(compute_newer_window_end(5, 10, 20), 15);
}

#[test]
fn test_compute_newer_window_end_at_boundary() {
    assert_eq!(compute_newer_window_end(15, 10, 20), 20);
}

#[test]
fn test_compute_newer_window_end_beyond() {
    assert_eq!(compute_newer_window_end(18, 10, 20), 20);
}

// ── sync_ollama_host_to_provider ──────────────────────────────────

#[test]
fn test_sync_ollama_host_ollama_with_host() {
    let mut provider = make_model_provider("http://original:11434");
    let mut config = XEChatConfig::default();
    config.model_provider = "ollama".to_string();
    config.preferences.ollama.host = "http://custom:11434".to_string();
    sync_ollama_host_to_provider(&mut provider, &config);
    assert_eq!(provider.base_url, "http://custom:11434");
}

#[test]
fn test_sync_ollama_host_ollama_without_host() {
    let mut provider = make_model_provider("http://original:11434");
    let mut config = XEChatConfig::default();
    config.model_provider = "ollama".to_string();
    config.preferences.ollama.host = String::new();
    sync_ollama_host_to_provider(&mut provider, &config);
    assert_eq!(provider.base_url, "http://original:11434");
}

#[test]
fn test_sync_ollama_host_non_ollama() {
    let mut provider = make_model_provider("http://original:11434");
    let mut config = XEChatConfig::default();
    config.model_provider = "openai".to_string();
    config.preferences.ollama.host = "http://custom:11434".to_string();
    sync_ollama_host_to_provider(&mut provider, &config);
    assert_eq!(provider.base_url, "http://original:11434");
}
