use std::path::PathBuf;
use std::sync::{OnceLock, Mutex};
use crate::models::error::AppError;

static TEST_APP_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

pub fn validate_conv_id(conv_id: &str) -> Result<(), AppError> {
    if conv_id.is_empty() {
        return Err(AppError::InvalidInput {
            field: "conv_id".into(),
            reason: "cannot be empty".into(),
        });
    }
    if conv_id.len() > 256 {
        return Err(AppError::InvalidInput {
            field: "conv_id".into(),
            reason: "exceeds 256 characters".into(),
        });
    }
    if !conv_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::InvalidInput {
            field: "conv_id".into(),
            reason: "contains invalid characters (only alphanumeric, -, _ allowed)".into(),
        });
    }
    Ok(())
}

/// 获取应用主目录路径，用于存储所有用户数据
pub fn get_app_dir() -> PathBuf {
    if let Some(lock) = TEST_APP_DIR.get()
        && let Some(dir) = lock.lock().unwrap().as_ref() {
            return dir.clone();
        }

    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."));

    #[cfg(target_os = "linux")]
    {
        base.join("xechat")
    }

    #[cfg(not(target_os = "linux"))]
    {
        base.join("XEChat")
    }
}

/// 获取应用配置文件 `config.toml` 的完整路径
pub fn get_config_path() -> PathBuf {
    get_app_dir().join("config.toml")
}

pub fn get_conversations_index_path() -> PathBuf {
    get_app_dir().join("conversations.json")
}

pub fn get_conversation_dir(conv_id: &str) -> PathBuf {
    validate_conv_id(conv_id).unwrap_or_else(|e| panic!("{}", e));
    get_app_dir().join("conversations").join(conv_id)
}

pub fn get_conversation_file(conv_id: &str) -> PathBuf {
    validate_conv_id(conv_id).unwrap_or_else(|e| panic!("{}", e));
    get_conversation_dir(conv_id).join("conversation.json")
}

pub fn ensure_app_dir() -> Result<(), AppError> {
    let dir = get_app_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(())
}

/// 确保配置目录存在（等同于确保应用主目录存在）
pub fn ensure_config_dir() -> Result<(), AppError> {
    ensure_app_dir()
}

fn get_legacy_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".xechat")
}

pub fn get_legacy_config_path() -> PathBuf {
    get_legacy_dir().join("config.toml")
}

pub fn get_legacy_conversations_path() -> PathBuf {
    get_legacy_dir().join("conversations.json")
}

pub fn set_test_dir(dir: PathBuf) {
    let mut lock = TEST_APP_DIR
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap();
    *lock = Some(dir);
}

/// 清除测试目录设置，恢复使用默认的应用目录
pub fn clear_test_dir() {
    let mut lock = TEST_APP_DIR
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap();
    *lock = None;
}
