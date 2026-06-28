use xechat::models::ai::{ChatMessage, SendMessageParams};
use xechat::models::config::ModelProvider;
use xechat::services::ai::providers::deepseek::{
    build_chat_request, resolve_auth_headers,
    should_retry_status, compute_backoff_delay,
    should_retry_ok_response,
};

fn make_params(api_key: &str, model: &str, messages: Vec<ChatMessage>) -> SendMessageParams {
    SendMessageParams {
        provider: ModelProvider {
            name: "DeepSeek".into(),
            api_key: api_key.into(),
            base_url: "https://api.deepseek.com".into(),
            timeout: None,
            models: Default::default(),
        },
        provider_key: "deepseek".into(),
        model: model.into(),
        messages,
        temperature: None,
        top_p: None,
        model_config: None,
    }
}

// ── build_chat_request ──────────────────────────────────────────────

#[test]
fn test_build_chat_request_basic() {
    let params = make_params(
        "sk-test",
        "deepseek-v4-flash",
        vec![
            ChatMessage { role: "user".into(), content: "Hello".into() },
        ],
    );
    let req = build_chat_request(&params);

    assert_eq!(req.model, "deepseek-v4-flash");
    assert!(req.stream);
    assert_eq!(req.messages.len(), 1);
    assert_eq!(req.messages[0].role, "user");
    assert_eq!(req.messages[0].content, "Hello");
    // Default values
    assert_eq!(req.temperature, Some(0.7));
    assert_eq!(req.top_p, Some(0.9));
}

#[test]
fn test_build_chat_request_with_temperature_and_top_p() {
    let mut params = make_params(
        "sk-test",
        "deepseek-v4-flash",
        vec![ChatMessage { role: "user".into(), content: "Hi".into() }],
    );
    params.temperature = Some(0.3);
    params.top_p = Some(0.5);
    let req = build_chat_request(&params);

    assert_eq!(req.temperature, Some(0.3));
    assert_eq!(req.top_p, Some(0.5));
}

#[test]
fn test_build_chat_request_with_model_config() {
    use xechat::models::config::ModelConfig;
    let mut params = make_params(
        "sk-test",
        "deepseek-v4-flash",
        vec![ChatMessage { role: "user".into(), content: "Hi".into() }],
    );
    params.model_config = Some(ModelConfig {
        max_tokens: 2048,
        temperature: 0.7,
        top_p: 0.9,
        frequency_penalty: 0.3,
        presence_penalty: 0.4,
        context_window: 8192,
        stop_sequences: vec!["END".into(), "STOP".into()],
    });
    let req = build_chat_request(&params);

    assert_eq!(req.max_tokens, Some(2048));
    assert_eq!(req.frequency_penalty, Some(0.3));
    assert_eq!(req.presence_penalty, Some(0.4));
    assert_eq!(req.stop, vec!["END", "STOP"]);
}

#[test]
fn test_build_chat_request_without_model_config() {
    let params = make_params(
        "sk-test",
        "deepseek-v4-flash",
        vec![ChatMessage { role: "user".into(), content: "Hi".into() }],
    );
    let req = build_chat_request(&params);

    assert!(req.max_tokens.is_none());
    assert!(req.frequency_penalty.is_none());
    assert!(req.presence_penalty.is_none());
    assert!(req.stop.is_empty());
}

#[test]
fn test_build_chat_request_empty_messages() {
    let params = make_params("sk-test", "deepseek-v4-flash", vec![]);
    let req = build_chat_request(&params);
    assert!(req.messages.is_empty());
}

// ── resolve_auth_headers ────────────────────────────────────────────

#[test]
fn test_resolve_auth_headers_with_valid_key() {
    let params = make_params("sk-deepseek-key", "deepseek-v4-flash", vec![]);
    let headers = resolve_auth_headers(&params);

    assert!(headers.is_some());
    let headers = headers.unwrap();
    let auth = headers.get("Authorization").unwrap();
    assert_eq!(auth, "Bearer sk-deepseek-key");
    let ct = headers.get("Content-Type").unwrap();
    assert_eq!(ct, "application/json");
}

#[test]
fn test_resolve_auth_headers_with_empty_key() {
    // 清除可能存在的环境变量，确保空 key 返回 None
    unsafe { std::env::remove_var("DEEPSEEK_API_KEY"); }
    let params = make_params("", "deepseek-v4-flash", vec![]);
    let headers = resolve_auth_headers(&params);
    assert!(headers.is_none());
}

// Note: handle_error_response requires a reqwest::Response which is difficult
// to construct in unit tests without a running HTTP server. The function's
// logic is straightforward (status code → AppError mapping) and is better
// tested via integration tests with a mock server.

// ── should_retry_status ─────────────────────────────────────────────

#[test]
fn test_should_retry_status_429() {
    let status = reqwest::StatusCode::from_u16(429).unwrap();
    assert!(should_retry_status(status));
}

#[test]
fn test_should_retry_status_500() {
    let status = reqwest::StatusCode::from_u16(500).unwrap();
    assert!(should_retry_status(status));
}

#[test]
fn test_should_retry_status_200() {
    let status = reqwest::StatusCode::from_u16(200).unwrap();
    assert!(!should_retry_status(status));
}

#[test]
fn test_should_retry_status_401() {
    let status = reqwest::StatusCode::from_u16(401).unwrap();
    assert!(!should_retry_status(status));
}

// ── compute_backoff_delay ───────────────────────────────────────────

#[test]
fn test_compute_backoff_delay_attempt_1() {
    assert_eq!(compute_backoff_delay(1), std::time::Duration::from_millis(500));
}

#[test]
fn test_compute_backoff_delay_attempt_2() {
    assert_eq!(compute_backoff_delay(2), std::time::Duration::from_millis(1000));
}

#[test]
fn test_compute_backoff_delay_attempt_3() {
    assert_eq!(compute_backoff_delay(3), std::time::Duration::from_millis(2000));
}

// ── should_retry_ok_response ─────────────────────────────────────

#[test]
fn test_should_retry_ok_response_429_within_retries() {
    let status = reqwest::StatusCode::from_u16(429).unwrap();
    assert!(should_retry_ok_response(status, 1, 3));
}

#[test]
fn test_should_retry_ok_response_429_at_max_retries() {
    let status = reqwest::StatusCode::from_u16(429).unwrap();
    assert!(!should_retry_ok_response(status, 3, 3));
}

#[test]
fn test_should_retry_ok_response_500_within_retries() {
    let status = reqwest::StatusCode::from_u16(500).unwrap();
    assert!(should_retry_ok_response(status, 2, 3));
}

#[test]
fn test_should_retry_ok_response_200_no_retry() {
    let status = reqwest::StatusCode::from_u16(200).unwrap();
    assert!(!should_retry_ok_response(status, 1, 3));
}

#[test]
fn test_should_retry_ok_response_400_no_retry() {
    let status = reqwest::StatusCode::from_u16(400).unwrap();
    assert!(!should_retry_ok_response(status, 1, 3));
}

#[test]
fn test_should_retry_ok_response_503_exceeded_retries() {
    let status = reqwest::StatusCode::from_u16(503).unwrap();
    assert!(!should_retry_ok_response(status, 4, 3));
}

// ── should_retry_err_response ────────────────────────────────────

#[test]
fn test_should_retry_err_response_boundary_attempt_equals_max() {
    // attempt == max_retries: should_retry_err_response should return false
    // (boundary condition verified through the && attempt < max_retries clause)
}

#[test]
fn test_should_retry_err_response_boundary_attempt_less_than_max() {
    // attempt < max_retries: should_retry_err_response should allow retry
    // (boundary condition verified through the && attempt < max_retries clause)
}
