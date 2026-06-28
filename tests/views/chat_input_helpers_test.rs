use std::collections::HashMap;
use xechat::models::config::{XEChatConfig, ModelProvider, ModelConfig};
use xechat::views::conversation::chat_input::{build_model_options, should_send, apply_model_selection};

fn make_model_config() -> ModelConfig {
    ModelConfig {
        max_tokens: 4096,
        temperature: 0.7,
        top_p: 0.95,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        context_window: 8192,
        stop_sequences: vec![],
    }
}

fn make_test_config() -> XEChatConfig {
    let mut config = XEChatConfig {
        model_provider: "deepseek".to_string(),
        model: "deepseek-chat".to_string(),
        ..Default::default()
    };

    let mut deepseek_models = HashMap::new();
    deepseek_models.insert("deepseek-chat".to_string(), make_model_config());
    deepseek_models.insert("deepseek-reasoner".to_string(), make_model_config());
    let deepseek_provider = ModelProvider {
        name: "DeepSeek".to_string(),
        api_key: "test-key".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        timeout: Some(120),
        models: deepseek_models,
    };
    config.model_providers.insert("deepseek".to_string(), deepseek_provider);

    let mut compatible_models = HashMap::new();
    compatible_models.insert("gpt-4o".to_string(), make_model_config());
    let compatible_provider = ModelProvider {
        name: "Custom".to_string(),
        api_key: "test-key".to_string(),
        base_url: "https://api.custom.com".to_string(),
        timeout: None,
        models: compatible_models,
    };
    config.model_providers.insert("custom-provider".to_string(), compatible_provider);

    config
}

// ── build_model_options ─────────────────────────────────────────

#[test]
fn test_build_model_options_returns_current_value() {
    let config = make_test_config();
    let (_options, current) = build_model_options(&config);
    assert_eq!(current, "deepseek/deepseek-chat");
}

#[test]
fn test_build_model_options_includes_all_providers() {
    let config = make_test_config();
    let (options, _) = build_model_options(&config);
    // deepseek has 2 models, custom-provider has 1 model
    assert_eq!(options.len(), 3);
}

#[test]
fn test_build_model_options_deepseek_no_compatible_label() {
    let config = make_test_config();
    let (options, _) = build_model_options(&config);
    let deepseek_opts: Vec<_> = options.iter()
        .filter(|(k, _)| k.starts_with("deepseek/"))
        .collect();
    assert_eq!(deepseek_opts.len(), 2);
    // DeepSeek models should NOT have the compatible label
    for (_, label) in deepseek_opts {
        assert!(!label.contains("OpenAI Compatible"));
    }
}

#[test]
fn test_build_model_options_compatible_provider_has_label() {
    let config = make_test_config();
    let (options, _) = build_model_options(&config);
    let compatible_opts: Vec<_> = options.iter()
        .filter(|(k, _)| k.starts_with("custom-provider/"))
        .collect();
    assert_eq!(compatible_opts.len(), 1);
    // Compatible provider models should have the compatible label
    let (_, label) = compatible_opts[0];
    assert!(label.contains("OpenAI Compatible"));
}

#[test]
fn test_build_model_options_value_format() {
    let config = make_test_config();
    let (options, _) = build_model_options(&config);
    for (key, _) in &options {
        assert!(key.contains('/'), "Key should be in 'provider/model' format, got: {}", key);
    }
}

#[test]
fn test_build_model_options_empty_config() {
    let config = XEChatConfig::default();
    let (options, _current) = build_model_options(&config);
    // Default config has providers with models
    // Just verify the format is consistent
    for (key, _) in &options {
        assert!(key.contains('/'), "Key should be in 'provider/model' format, got: {}", key);
    }
}

// ── should_send ─────────────────────────────────────────────────

#[test]
fn test_should_send_with_content_and_not_streaming() {
    assert!(should_send("hello", false));
}

#[test]
fn test_should_send_with_empty_content() {
    assert!(!should_send("", false));
}

#[test]
fn test_should_send_with_whitespace_only() {
    assert!(!should_send("   ", false));
}

#[test]
fn test_should_send_while_streaming() {
    assert!(!should_send("hello", true));
}

#[test]
fn test_should_send_empty_while_streaming() {
    assert!(!should_send("", true));
}

// ── apply_model_selection ───────────────────────────────────────

#[test]
fn test_apply_model_selection_valid_format() {
    let mut config = XEChatConfig::default();
    let result = apply_model_selection("ollama/llama3", &mut config);
    assert!(result.is_some());
    assert_eq!(config.model_provider, "ollama");
    assert_eq!(config.model, "llama3");
}

#[test]
fn test_apply_model_selection_no_slash() {
    let mut config = XEChatConfig::default();
    let result = apply_model_selection("invalid", &mut config);
    assert!(result.is_none());
}

#[test]
fn test_apply_model_selection_empty_provider() {
    let mut config = XEChatConfig::default();
    let result = apply_model_selection("/model", &mut config);
    assert!(result.is_some());
    assert_eq!(config.model_provider, "");
    assert_eq!(config.model, "model");
}

#[test]
fn test_apply_model_selection_empty_model() {
    let mut config = XEChatConfig::default();
    let result = apply_model_selection("provider/", &mut config);
    assert!(result.is_some());
    assert_eq!(config.model_provider, "provider");
    assert_eq!(config.model, "");
}

#[test]
fn test_apply_model_selection_multiple_slashes() {
    let mut config = XEChatConfig::default();
    let result = apply_model_selection("provider/model/sub", &mut config);
    assert!(result.is_some());
    assert_eq!(config.model_provider, "provider");
    assert_eq!(config.model, "model/sub");
}
