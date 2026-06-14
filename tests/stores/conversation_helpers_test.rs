use xechat::stores::conversation::should_enable_ollama;
use xechat::stores::ConversationStore;
use xechat::models::config::XEChatConfig;

// ── should_enable_ollama ────────────────────────────────────────

#[test]
fn test_should_enable_ollama_with_ollama_provider_and_model() {
    let mut config = XEChatConfig::default();
    config.preferences.embed_provider = "ollama".to_string();
    config.preferences.ollama.embed_model = "nomic-embed-text".to_string();
    assert!(should_enable_ollama(&config));
}

#[test]
fn test_should_enable_ollama_wrong_provider() {
    let mut config = XEChatConfig::default();
    config.preferences.embed_provider = "e5".to_string();
    config.preferences.ollama.embed_model = "nomic-embed-text".to_string();
    assert!(!should_enable_ollama(&config));
}

#[test]
fn test_should_enable_ollama_empty_model() {
    let mut config = XEChatConfig::default();
    config.preferences.embed_provider = "ollama".to_string();
    config.preferences.ollama.embed_model = String::new();
    assert!(!should_enable_ollama(&config));
}

#[test]
fn test_should_enable_ollama_both_wrong() {
    let config = XEChatConfig::default();
    assert!(!should_enable_ollama(&config));
}

// ── resolve_ollama_host ─────────────────────────────────────────

#[test]
fn test_resolve_ollama_host_empty_string() {
    assert_eq!(
        ConversationStore::resolve_ollama_host(""),
        "http://localhost:11434"
    );
}

#[test]
fn test_resolve_ollama_host_custom_host() {
    assert_eq!(
        ConversationStore::resolve_ollama_host("http://192.168.1.100:11434"),
        "http://192.168.1.100:11434"
    );
}

#[test]
fn test_resolve_ollama_host_localhost_with_port() {
    assert_eq!(
        ConversationStore::resolve_ollama_host("http://localhost:8080"),
        "http://localhost:8080"
    );
}

#[test]
fn test_resolve_ollama_host_https() {
    assert_eq!(
        ConversationStore::resolve_ollama_host("https://ollama.example.com"),
        "https://ollama.example.com"
    );
}
