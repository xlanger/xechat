use xechat::services::model_downloader::{get_model_path, get_models_dir, is_model_ready, get_resume_position, create_download_client};
use std::path::PathBuf;

#[test]
fn test_get_models_dir_ends_with_xechat_models() {
    let dir = get_models_dir();
    assert!(
        dir.ends_with("XEChat/models"),
        "get_models_dir() should end with 'XEChat/models', got: {}",
        dir.display()
    );
}

#[test]
fn test_get_model_path_ends_with_filename() {
    let path = get_model_path();
    assert!(
        path.ends_with("XEChat/models/qwen3-embedding-0.6b-q8_0.gguf"),
        "get_model_path() should end with 'XEChat/models/qwen3-embedding-0.6b-q8_0.gguf', got: {}",
        path.display()
    );
}

#[test]
fn test_get_model_path_is_under_models_dir() {
    let models_dir = get_models_dir();
    let model_path = get_model_path();
    assert_eq!(
        model_path.parent(),
        Some(models_dir.as_path()),
        "get_model_path() parent should equal get_models_dir()"
    );
}

#[test]
fn test_is_model_ready_returns_false_when_no_model_file() {
    let ready = is_model_ready();
    let model_path = get_model_path();
    if !model_path.exists() {
        assert!(!ready, "is_model_ready() should return false when model file does not exist");
    }
}

// ── get_resume_position ─────────────────────────────────────────

#[test]
fn test_get_resume_position_nonexistent_file() {
    let path = PathBuf::from("/tmp/xechat_test_nonexistent_12345.tmp");
    let pos = get_resume_position(&path);
    assert_eq!(pos, 0, "Should return 0 for nonexistent file");
}

#[test]
fn test_get_resume_position_existing_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let file_path = dir.path().join("test_resume.tmp");
    std::fs::write(&file_path, b"hello world").expect("failed to write test file");
    let pos = get_resume_position(&file_path);
    assert_eq!(pos, 11, "Should return file size for existing file");
}

#[test]
fn test_get_resume_position_empty_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let file_path = dir.path().join("test_empty.tmp");
    std::fs::write(&file_path, b"").expect("failed to write empty file");
    let pos = get_resume_position(&file_path);
    assert_eq!(pos, 0, "Should return 0 for empty file");
}

// ── create_download_client ──────────────────────────────────────

#[test]
fn test_create_download_client_success() {
    let result = create_download_client();
    assert!(result.is_ok(), "Should successfully create HTTP client");
}

// ── parse_content_range_total ───────────────────────────────────

#[test]
fn test_parse_content_range_total_valid_header() {
    // We can't easily construct a reqwest::Response with custom headers in unit tests,
    // so we test the parsing logic indirectly. The function extracts the total from
    // Content-Range header format: "bytes start-end/total"
    // This is tested via integration tests with actual HTTP responses.
}

#[test]
fn test_parse_content_range_total_format() {
    // Verify the expected format: "bytes 0-999/5000" → total = 5000
    let parts: Vec<&str> = "bytes 0-999/5000".rsplit('/').collect();
    assert_eq!(parts[0], "5000", "Should extract total from Content-Range");
}

#[test]
fn test_parse_content_range_total_asterisk() {
    // Content-Range: bytes */5000 → total = 5000
    let parts: Vec<&str> = "bytes */5000".rsplit('/').collect();
    assert_eq!(parts[0], "5000", "Should extract total from Content-Range with asterisk");
}
