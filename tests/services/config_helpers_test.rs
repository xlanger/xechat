use xechat::services::config::read_config_file;
use std::io::Write;

// ── read_config_file ────────────────────────────────────────────

#[test]
fn test_read_config_file_not_exists() {
    let path = std::env::temp_dir().join("xechat_test_config_nonexistent_99999.toml");
    let result = read_config_file(&path);
    assert!(result.is_none());
}

#[test]
fn test_read_config_file_valid() {
    let dir = std::env::temp_dir().join("xechat_test_config_read");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("valid.toml");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, "model = \"gpt-4\"\nmodel_provider = \"openai\"\ntheme = \"dark\"\nlanguage = \"zh\"\ntimezone = \"system\"").unwrap();

    let result = read_config_file(&path);
    assert!(result.is_some());
    let config = result.unwrap();
    assert_eq!(config.model, "gpt-4");
    assert_eq!(config.model_provider, "openai");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_read_config_file_invalid_toml() {
    let dir = std::env::temp_dir().join("xechat_test_config_invalid");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("invalid.toml");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, "this is not valid toml {{{{").unwrap();

    let result = read_config_file(&path);
    assert!(result.is_none());

    std::fs::remove_dir_all(&dir).ok();
}
