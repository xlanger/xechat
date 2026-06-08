use xechat::utils::paths;
use xechat::models::error::AppError;
use tempfile::TempDir;

fn setup_temp_dir() -> TempDir {
    let temp = tempfile::tempdir().expect("failed to create temp dir");
    let path = temp.path().to_path_buf();
    paths::set_test_dir(path);
    temp
}

fn clear_temp_dir() {
    paths::clear_test_dir();
}

#[test]
fn test_validate_conv_id_valid() {
    assert!(paths::validate_conv_id("abc123").is_ok());
    assert!(paths::validate_conv_id("my-conversation").is_ok());
    assert!(paths::validate_conv_id("conv_2024").is_ok());
    assert!(paths::validate_conv_id("a").is_ok());
}

#[test]
fn test_validate_conv_id_empty() {
    let result = paths::validate_conv_id("");
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::InvalidInput { field, reason } => {
            assert_eq!(field, "conv_id");
            assert!(reason.contains("empty"));
        }
        other => panic!("expected InvalidInput, got {:?}", other),
    }
}

#[test]
fn test_validate_conv_id_too_long() {
    let long_id = "a".repeat(257);
    let result = paths::validate_conv_id(&long_id);
    assert!(result.is_err());
    match result.unwrap_err() {
        AppError::InvalidInput { field, reason } => {
            assert_eq!(field, "conv_id");
            assert!(reason.contains("256"));
        }
        other => panic!("expected InvalidInput, got {:?}", other),
    }
}

#[test]
fn test_validate_conv_id_max_length() {
    let max_id = "a".repeat(256);
    assert!(paths::validate_conv_id(&max_id).is_ok());
}

#[test]
fn test_validate_conv_id_invalid_chars() {
    let cases = ["abc def", "conv/id", "conv.id", "conv@id", "conv#id"];
    for case in cases {
        let result = paths::validate_conv_id(case);
        assert!(result.is_err(), "expected '{}' to be invalid", case);
        match result.unwrap_err() {
            AppError::InvalidInput { field, reason } => {
                assert_eq!(field, "conv_id");
                assert!(reason.contains("invalid characters"));
            }
            other => panic!("expected InvalidInput, got {:?}", other),
        }
    }
}

#[test]
fn test_validate_conv_id_hyphen_and_underscore() {
    assert!(paths::validate_conv_id("a-b_c").is_ok());
    assert!(paths::validate_conv_id("_-").is_ok());
}

#[test]
fn test_get_app_dir_with_test_dir() {
    let temp = setup_temp_dir();
    let app_dir = paths::get_app_dir();
    assert_eq!(app_dir, temp.path().to_path_buf());
    clear_temp_dir();
}

#[test]
fn test_get_config_path() {
    let temp = setup_temp_dir();
    let config_path = paths::get_config_path();
    assert_eq!(config_path, temp.path().join("config.toml"));
    clear_temp_dir();
}

#[test]
fn test_get_conversations_index_path() {
    let temp = setup_temp_dir();
    let index_path = paths::get_conversations_index_path();
    assert_eq!(index_path, temp.path().join("conversations.json"));
    clear_temp_dir();
}

#[test]
fn test_get_conversation_dir() {
    let temp = setup_temp_dir();
    let conv_dir = paths::get_conversation_dir("test-conv");
    assert_eq!(conv_dir, temp.path().join("conversations").join("test-conv"));
    clear_temp_dir();
}

#[test]
fn test_get_conversation_file() {
    let temp = setup_temp_dir();
    let conv_file = paths::get_conversation_file("test-conv");
    assert_eq!(
        conv_file,
        temp.path().join("conversations").join("test-conv").join("conversation.json")
    );
    clear_temp_dir();
}

#[test]
fn test_get_conversation_dir_invalid_id_panics() {
    let result = std::panic::catch_unwind(|| {
        paths::get_conversation_dir("invalid/id");
    });
    assert!(result.is_err());
}

#[test]
fn test_ensure_app_dir() {
    let temp = setup_temp_dir();
    let result = paths::ensure_app_dir();
    assert!(result.is_ok());
    assert!(temp.path().exists());
    clear_temp_dir();
}

#[test]
fn test_ensure_config_dir() {
    let _temp = setup_temp_dir();
    let result = paths::ensure_config_dir();
    assert!(result.is_ok());
    clear_temp_dir();
}

#[test]
fn test_get_legacy_config_path() {
    let path = paths::get_legacy_config_path();
    assert!(path.to_string_lossy().contains(".xechat"));
    assert!(path.to_string_lossy().contains("config.toml"));
}

#[test]
fn test_get_legacy_conversations_path() {
    let path = paths::get_legacy_conversations_path();
    assert!(path.to_string_lossy().contains(".xechat"));
    assert!(path.to_string_lossy().contains("conversations.json"));
}
