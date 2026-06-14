use xechat::services::conversation::{read_conversation_from_file, read_conversation_or_default, load_conversation_mut};
use std::io::Write;

fn make_conversation_json(id: &str, title: &str) -> String {
    serde_json::json!({
        "id": id,
        "title": title,
        "messages": [],
        "created_at": "2024-01-01T00:00:00Z",
        "updated_at": "2024-01-01T00:00:00Z",
        "is_temporary": false
    }).to_string()
}

// ── read_conversation_from_file ─────────────────────────────────

#[test]
fn test_read_conversation_from_file_not_exists() {
    let path = std::env::temp_dir().join("xechat_test_nonexistent_12345.json");
    let result = read_conversation_from_file(&path);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_read_conversation_from_file_valid() {
    let dir = std::env::temp_dir().join("xechat_test_read_conv");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("valid.json");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, "{}", make_conversation_json("conv1", "Test")).unwrap();

    let result = read_conversation_from_file(&path);
    assert!(result.is_ok());
    let conv = result.unwrap().unwrap();
    assert_eq!(conv.id, "conv1");
    assert_eq!(conv.title, "Test");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_read_conversation_from_file_invalid_json() {
    let dir = std::env::temp_dir().join("xechat_test_read_conv_invalid");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("invalid.json");
    let mut file = std::fs::File::create(&path).unwrap();
    write!(file, "not json").unwrap();

    let result = read_conversation_from_file(&path);
    assert!(result.is_err());

    std::fs::remove_dir_all(&dir).ok();
}

// ── read_conversation_or_default ────────────────────────────────

#[test]
fn test_read_conversation_or_default_returns_default_on_missing() {
    // This test depends on paths::get_conversation_file which uses the app data dir.
    // Since the file won't exist, it should return a default conversation.
    let result = read_conversation_or_default("nonexistent_conv_99999", "Fallback Title");
    assert_eq!(result.id, "nonexistent_conv_99999");
    assert_eq!(result.title, "Fallback Title");
    assert!(result.messages.is_empty());
}

// ── load_conversation_mut ───────────────────────────────────────

#[test]
fn test_load_conversation_mut_not_found() {
    let result = load_conversation_mut("nonexistent_conv_99999");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}
