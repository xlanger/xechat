use xechat::models::config::{ModelProvider, resolve_env_vars_in_str};
use std::collections::HashMap;

#[test]
fn test_resolve_env_vars_in_str_braced() {
    unsafe { std::env::set_var("TEST_MODEL_KEY_BRACED", "sk-test-123"); }
    let input = "key=${TEST_MODEL_KEY_BRACED}";
    let result = resolve_env_vars_in_str(input);
    assert_eq!(result, "key=sk-test-123");
    unsafe { std::env::remove_var("TEST_MODEL_KEY_BRACED"); }
}

#[test]
fn test_resolve_env_vars_in_str_simple() {
    unsafe { std::env::set_var("TEST_MODEL_KEY_SIMPLE", "sk-simple-456"); }
    let input = "key=$TEST_MODEL_KEY_SIMPLE";
    let result = resolve_env_vars_in_str(input);
    assert_eq!(result, "key=sk-simple-456");
    unsafe { std::env::remove_var("TEST_MODEL_KEY_SIMPLE"); }
}

#[test]
fn test_resolve_env_vars_in_str_no_match() {
    let input = "plain text no vars";
    let result = resolve_env_vars_in_str(input);
    assert_eq!(result, "plain text no vars");
}

#[test]
fn test_resolve_env_vars_in_str_missing_keeps_original() {
    unsafe { std::env::remove_var("NONEXISTENT_MODEL_VAR_XYZ"); }
    let input = "prefix_${NONEXISTENT_MODEL_VAR_XYZ}_suffix";
    let result = resolve_env_vars_in_str(input);
    assert_eq!(result, "prefix_${NONEXISTENT_MODEL_VAR_XYZ}_suffix");
}

#[test]
fn test_resolve_api_key_from_config_value() {
    unsafe { std::env::remove_var("TEST_API_CONFIG_API_KEY"); }
    let provider = ModelProvider {
        name: "TestApiConfig".to_string(),
        api_key: "sk-config-123".to_string(),
        base_url: "https://api.test.com".to_string(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_api_key("test_api_config"), Some("sk-config-123".to_string()));
}

#[test]
fn test_resolve_api_key_from_env() {
    unsafe { std::env::set_var("DEEPSEEK_API_KEY", "sk-env-789"); }
    let provider = ModelProvider {
        name: "DeepSeek".to_string(),
        api_key: String::new(),
        base_url: "https://api.deepseek.com".to_string(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_api_key("deepseek"), Some("sk-env-789".to_string()));
    unsafe { std::env::remove_var("DEEPSEEK_API_KEY"); }
}

#[test]
fn test_resolve_api_key_with_env_var_reference() {
    unsafe { std::env::set_var("MY_SECRET_KEY", "sk-secret-abc"); }
    let provider = ModelProvider {
        name: "Test".to_string(),
        api_key: "${MY_SECRET_KEY}".to_string(),
        base_url: String::new(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_api_key("test"), Some("sk-secret-abc".to_string()));
    unsafe { std::env::remove_var("MY_SECRET_KEY"); }
}

#[test]
fn test_resolve_api_key_none_when_empty() {
    unsafe { std::env::remove_var("MISSING_PROVIDER_API_KEY"); }
    let provider = ModelProvider {
        name: "Missing".to_string(),
        api_key: String::new(),
        base_url: String::new(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_api_key("missing-provider"), None);
}

#[test]
fn test_resolve_base_url_from_config_value() {
    unsafe { std::env::remove_var("TEST_URL_CONFIG_BASE_URL"); }
    let provider = ModelProvider {
        name: "TestUrlConfig".to_string(),
        api_key: String::new(),
        base_url: "https://custom.test.com".to_string(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_base_url("test_url_config"), Some("https://custom.test.com".to_string()));
}

#[test]
fn test_resolve_base_url_from_env() {
    unsafe { std::env::set_var("DEEPSEEK_BASE_URL", "https://env.deepseek.com"); }
    let provider = ModelProvider {
        name: "DeepSeek".to_string(),
        api_key: String::new(),
        base_url: String::new(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_base_url("deepseek"), Some("https://env.deepseek.com".to_string()));
    unsafe { std::env::remove_var("DEEPSEEK_BASE_URL"); }
}

#[test]
fn test_resolve_base_url_none_when_empty() {
    unsafe { std::env::remove_var("MISSING_PROVIDER_BASE_URL"); }
    let provider = ModelProvider {
        name: "Missing".to_string(),
        api_key: String::new(),
        base_url: String::new(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_base_url("missing-provider"), None);
}

#[test]
fn test_resolve_api_key_dash_to_underscore() {
    unsafe { std::env::set_var("MY_PROVIDER_API_KEY", "sk-dash-key"); }
    let provider = ModelProvider {
        name: "My".to_string(),
        api_key: String::new(),
        base_url: String::new(),
        timeout: None,
        models: HashMap::new(),
    };
    assert_eq!(provider.resolve_api_key("my-provider"), Some("sk-dash-key".to_string()));
    unsafe { std::env::remove_var("MY_PROVIDER_API_KEY"); }
}
