use xechat::models::ai::{
    ChatMessage, ChatRequest, ChatResponse,
    StreamEvent, SendMessageParams,
    DEFAULT_MAX_CONTEXT_TOKENS, DEFAULT_AUTO_CONTEXT_MANAGEMENT, DEFAULT_MAX_CONTEXT_MESSAGES,
};
use xechat::models::config::ModelProvider;
use xechat::models::error::AppError;
use std::collections::HashMap;

#[test]
fn test_chat_message_serialization() {
    let msg = ChatMessage {
        role: "user".to_string(),
        content: "Hello, world!".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"role\":\"user\""));
    assert!(json.contains("\"content\":\"Hello, world!\""));
}

#[test]
fn test_chat_message_system_role() {
    let msg = ChatMessage {
        role: "system".to_string(),
        content: "You are a helpful assistant.".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"role\":\"system\""));
}

#[test]
fn test_chat_message_assistant_role() {
    let msg = ChatMessage {
        role: "assistant".to_string(),
        content: "Sure, I can help.".to_string(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"role\":\"assistant\""));
}

#[test]
fn test_chat_message_empty_content() {
    let msg = ChatMessage {
        role: "user".to_string(),
        content: String::new(),
    };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"content\":\"\""));
}

#[test]
fn test_chat_request_serialization_with_stream() {
    let req = ChatRequest {
        model: "deepseek-v4-flash".to_string(),
        messages: vec![
            ChatMessage { role: "system".to_string(), content: "Be concise.".to_string() },
            ChatMessage { role: "user".to_string(), content: "Hi".to_string() },
        ],
        stream: true,
        temperature: None,
        top_p: None,
        max_tokens: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: vec![],
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"stream\":true"));
    assert!(json.contains("\"model\":\"deepseek-v4-flash\""));
    assert!(!json.contains("temperature"));
    assert!(!json.contains("top_p"));
    assert!(!json.contains("max_tokens"));
}

#[test]
fn test_chat_request_serialization_with_temperature() {
    let req = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        stream: false,
        temperature: Some(0.7),
        top_p: None,
        max_tokens: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: vec![],
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"temperature\":0.7"));
    assert!(!json.contains("top_p"));
}

#[test]
fn test_chat_request_serialization_with_top_p() {
    let req = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        stream: false,
        temperature: None,
        top_p: Some(0.9),
        max_tokens: None,
        frequency_penalty: None,
        presence_penalty: None,
        stop: vec![],
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"top_p\":0.9"));
    assert!(!json.contains("temperature"));
}

#[test]
fn test_chat_request_serialization_with_all_fields() {
    let req = ChatRequest {
        model: "test-model".to_string(),
        messages: vec![
            ChatMessage { role: "user".to_string(), content: "test".to_string() },
        ],
        stream: true,
        temperature: Some(1.0),
        top_p: Some(0.5),
        max_tokens: Some(4096),
        frequency_penalty: Some(0.5),
        presence_penalty: Some(0.3),
        stop: vec!["\n".to_string(), "###".to_string()],
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("\"temperature\":1.0"));
    assert!(json.contains("\"top_p\":0.5"));
    assert!(json.contains("\"stream\":true"));
    assert!(json.contains("\"max_tokens\":4096"));
    assert!(json.contains("\"frequency_penalty\":0.5"));
    assert!(json.contains("\"presence_penalty\":0.3"));
    assert!(json.contains("\"stop\""));
}

#[test]
fn test_chat_response_deserialization() {
    let json = r#"{"choices":[{"delta":{"content":"Hello"}}]}"#;
    let resp: ChatResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.choices.len(), 1);
    assert_eq!(resp.choices[0].delta.as_ref().unwrap().content.as_deref(), Some("Hello"));
}

#[test]
fn test_chat_response_multiple_choices() {
    let json = r#"{"choices":[{"delta":{"content":"A"}},{"delta":{"content":"B"}}]}"#;
    let resp: ChatResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.choices.len(), 2);
}

#[test]
fn test_chat_choice_delta_none() {
    let json = r#"{"choices":[{"delta":null}]}"#;
    let resp: ChatResponse = serde_json::from_str(json).unwrap();
    assert!(resp.choices[0].delta.is_none());
}

#[test]
fn test_chat_delta_content_none() {
    let json = r#"{"choices":[{"delta":{"content":null}}]}"#;
    let resp: ChatResponse = serde_json::from_str(json).unwrap();
    assert!(resp.choices[0].delta.as_ref().unwrap().content.is_none());
}

#[test]
fn test_stream_event_chunk() {
    let event = StreamEvent::Chunk("Hello".to_string());
    match event {
        StreamEvent::Chunk(text) => assert_eq!(text, "Hello"),
        _ => panic!("expected Chunk variant"),
    }
}

#[test]
fn test_stream_event_complete() {
    let event = StreamEvent::Complete;
    match event {
        StreamEvent::Complete => {}
        _ => panic!("expected Complete variant"),
    }
}

#[test]
fn test_stream_event_error() {
    let event = StreamEvent::Error(AppError::Network { detail: "timeout".to_string() });
    match event {
        StreamEvent::Error(AppError::Network { detail }) => {
            assert_eq!(detail, "timeout");
        }
        _ => panic!("expected Error variant"),
    }
}

#[test]
fn test_stream_event_clone() {
    let event = StreamEvent::Chunk("test".to_string());
    let cloned = event.clone();
    match cloned {
        StreamEvent::Chunk(text) => assert_eq!(text, "test"),
        _ => panic!("expected Chunk variant after clone"),
    }
}

#[test]
fn test_default_max_context_tokens() {
    assert_eq!(DEFAULT_MAX_CONTEXT_TOKENS, 8192);
}

#[test]
fn test_default_auto_context_management() {
    assert!(DEFAULT_AUTO_CONTEXT_MANAGEMENT);
}

#[test]
fn test_default_max_context_messages() {
    assert_eq!(DEFAULT_MAX_CONTEXT_MESSAGES, 20);
}

#[test]
fn test_send_message_params_construction() {
    let provider = ModelProvider {
        name: "TestProvider".to_string(),
        api_key: "sk-test".to_string(),
        base_url: "https://api.test.com".to_string(),
        timeout: Some(30),
        models: HashMap::new(),
    };
    let params = SendMessageParams {
        provider: provider.clone(),
        provider_key: "test_provider".to_string(),
        model: "test-model".to_string(),
        messages: vec![
            ChatMessage { role: "user".to_string(), content: "Hi".to_string() },
        ],
        temperature: Some(0.5),
        top_p: None,
        model_config: None,
    };
    assert_eq!(params.provider_key, "test_provider");
    assert_eq!(params.model, "test-model");
    assert_eq!(params.messages.len(), 1);
    assert_eq!(params.temperature, Some(0.5));
    assert_eq!(params.top_p, None);
    assert_eq!(params.provider.name, "TestProvider");
}

#[test]
fn test_send_message_params_clone() {
    let provider = ModelProvider {
        name: "P".to_string(),
        api_key: String::new(),
        base_url: String::new(),
        timeout: None,
        models: HashMap::new(),
    };
    let params = SendMessageParams {
        provider,
        provider_key: "p".to_string(),
        model: "m".to_string(),
        messages: vec![],
        temperature: None,
        top_p: None,
        model_config: None,
    };
    let cloned = params.clone();
    assert_eq!(cloned.provider_key, "p");
    assert_eq!(cloned.model, "m");
}
