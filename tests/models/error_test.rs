use xechat::models::error::{AppError, AuthFailReason};

#[test]
fn test_display_network_error() {
    let err = AppError::Network { detail: "connection timed out".to_string() };
    assert_eq!(format!("{}", err), "Network error: connection timed out");
}

#[test]
fn test_display_auth_invalid_key_format() {
    let err = AppError::Auth { reason: AuthFailReason::InvalidKeyFormat };
    assert_eq!(format!("{}", err), "Authentication error: invalid API key format");
}

#[test]
fn test_display_auth_unauthorized() {
    let err = AppError::Auth {
        reason: AuthFailReason::Unauthorized { config_path: "/path/to/config".to_string() },
    };
    assert_eq!(format!("{}", err), "Authentication error: unauthorized (401)");
}

#[test]
fn test_display_api_error_with_body() {
    let err = AppError::Api {
        status: 429,
        body: Some("rate limit exceeded".to_string()),
    };
    assert_eq!(format!("{}", err), "API error (429): rate limit exceeded");
}

#[test]
fn test_display_api_error_without_body() {
    let err = AppError::Api { status: 500, body: None };
    assert_eq!(format!("{}", err), "API error (500)");
}

#[test]
fn test_display_stream_error() {
    let err = AppError::Stream { detail: "connection lost".to_string() };
    assert_eq!(format!("{}", err), "Stream error: connection lost");
}

#[test]
fn test_display_config_error() {
    let err = AppError::Config {
        operation: "parse".to_string(),
        detail: "invalid TOML".to_string(),
    };
    assert_eq!(format!("{}", err), "Config error (parse): invalid TOML");
}

#[test]
fn test_display_io_error() {
    let err = AppError::Io {
        operation: "write file".to_string(),
        detail: "permission denied".to_string(),
    };
    assert_eq!(format!("{}", err), "IO error (write file): permission denied");
}

#[test]
fn test_display_serialization_error() {
    let err = AppError::Serialization {
        format: "json".to_string(),
        detail: "unexpected token".to_string(),
    };
    assert_eq!(format!("{}", err), "Serialization error (json): unexpected token");
}

#[test]
fn test_display_invalid_input_error() {
    let err = AppError::InvalidInput {
        field: "api_key".to_string(),
        reason: "cannot be empty".to_string(),
    };
    assert_eq!(format!("{}", err), "Invalid input 'api_key': cannot be empty");
}

#[test]
fn test_display_unsupported_error() {
    let err = AppError::Unsupported { item: "gRPC".to_string() };
    assert_eq!(format!("{}", err), "Unsupported: gRPC");
}

#[test]
fn test_i18n_key_network() {
    let err = AppError::Network { detail: "timeout".to_string() };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.network");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, "detail");
    assert_eq!(args[0].1, "timeout");
}

#[test]
fn test_i18n_key_auth_invalid_key_format() {
    let err = AppError::Auth { reason: AuthFailReason::InvalidKeyFormat };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.auth.invalidKey");
    assert!(args.is_empty());
}

#[test]
fn test_i18n_key_auth_unauthorized() {
    let err = AppError::Auth {
        reason: AuthFailReason::Unauthorized { config_path: "/etc/xechat".to_string() },
    };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.auth.unauthorized");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, "path");
    assert_eq!(args[0].1, "/etc/xechat");
}

#[test]
fn test_i18n_key_api_with_body() {
    let err = AppError::Api {
        status: 429,
        body: Some("rate limited".to_string()),
    };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.api.httpError");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].0, "status");
    assert_eq!(args[0].1, "429");
    assert_eq!(args[1].0, "body");
    assert_eq!(args[1].1, "rate limited");
}

#[test]
fn test_i18n_key_api_without_body() {
    let err = AppError::Api { status: 500, body: None };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.api.httpError");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, "status");
    assert_eq!(args[0].1, "500");
}

#[test]
fn test_i18n_key_stream() {
    let err = AppError::Stream { detail: "broken pipe".to_string() };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.stream.readError");
    assert_eq!(args[0].0, "detail");
    assert_eq!(args[0].1, "broken pipe");
}

#[test]
fn test_i18n_key_config() {
    let err = AppError::Config {
        operation: "read".to_string(),
        detail: "file not found".to_string(),
    };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.config.failed");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].0, "operation");
    assert_eq!(args[1].0, "detail");
}

#[test]
fn test_i18n_key_io() {
    let err = AppError::Io {
        operation: "create directory".to_string(),
        detail: "disk full".to_string(),
    };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.io.failed");
    assert_eq!(args.len(), 2);
}

#[test]
fn test_i18n_key_serialization() {
    let err = AppError::Serialization {
        format: "toml".to_string(),
        detail: "parse error".to_string(),
    };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.serialization.parseError");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].0, "format");
    assert_eq!(args[1].0, "detail");
}

#[test]
fn test_i18n_key_invalid_input() {
    let err = AppError::InvalidInput {
        field: "email".to_string(),
        reason: "invalid format".to_string(),
    };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.invalidInput");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].0, "field");
    assert_eq!(args[1].0, "reason");
}

#[test]
fn test_i18n_key_unsupported() {
    let err = AppError::Unsupported { item: "FTP".to_string() };
    let (key, args) = err.i18n_key();
    assert_eq!(key, "error.unsupported");
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].0, "item");
    assert_eq!(args[0].1, "FTP");
}

#[test]
fn test_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let app_err: AppError = io_err.into();
    match app_err {
        AppError::Io { operation, detail } => {
            assert_eq!(operation, "fs");
            assert!(detail.contains("file not found"));
        }
        other => panic!("expected Io variant, got {:?}", other),
    }
}

#[test]
fn test_from_serde_json_error() {
    let json_err = serde_json::from_str::<serde_json::Value>("{invalid}");
    let app_err: AppError = json_err.unwrap_err().into();
    match app_err {
        AppError::Serialization { format, .. } => {
            assert_eq!(format, "json");
        }
        other => panic!("expected Serialization variant, got {:?}", other),
    }
}

#[test]
fn test_from_toml_de_error() {
    let toml_err = toml::from_str::<toml::Value>("= invalid");
    let app_err: AppError = toml_err.unwrap_err().into();
    match app_err {
        AppError::Config { operation, .. } => {
            assert_eq!(operation, "parse");
        }
        other => panic!("expected Config variant, got {:?}", other),
    }
}

#[test]
fn test_app_error_implements_std_error() {
    let err = AppError::Network { detail: "test".to_string() };
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_app_error_is_clone() {
    let err = AppError::Network { detail: "test".to_string() };
    let cloned = err.clone();
    assert_eq!(format!("{}", err), format!("{}", cloned));
}
