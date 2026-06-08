use std::path::PathBuf;
use std::sync::{OnceLock, Mutex};

static TEST_APP_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

pub fn set_test_dir(dir: PathBuf) {
    let mut lock = TEST_APP_DIR
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap();
    *lock = Some(dir);
}

pub fn clear_test_dir() {
    let mut lock = TEST_APP_DIR
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap();
    *lock = None;
}

pub fn get_app_dir() -> PathBuf {
    if let Some(lock) = TEST_APP_DIR.get()
        && let Some(dir) = lock.lock().unwrap().as_ref() {
            return dir.clone();
        }

    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."));

    base.join("XEChat")
}

/// 获取配置文件路径。
///
/// # Returns
///
/// `{app_dir}/config.toml` 的绝对路径。
pub fn get_config_path() -> PathBuf {
    get_app_dir().join("config.toml")
}

/// 获取对话索引文件路径。
///
/// # Returns
///
/// `{app_dir}/conversations.json` 的绝对路径。
pub fn get_conversations_index_path() -> PathBuf {
    get_app_dir().join("conversations.json")
}

/// 获取指定对话的存储目录路径。
///
/// # Arguments
///
/// * `conv_id` - 对话唯一标识符
///
/// # Returns
///
/// `{app_dir}/conversations/{conv_id}` 的绝对路径。
pub fn get_conversation_dir(conv_id: &str) -> PathBuf {
    get_app_dir().join("conversations").join(conv_id)
}

/// 获取指定对话的 JSON 文件路径。
///
/// # Arguments
///
/// * `conv_id` - 对话唯一标识符
///
/// # Returns
///
/// `{app_dir}/conversations/{conv_id}/conversation.json` 的绝对路径。
pub fn get_conversation_file(conv_id: &str) -> PathBuf {
    get_conversation_dir(conv_id).join("conversation.json")
}

/// 确保应用数据目录存在。
///
/// 若目录不存在则递归创建。
///
/// # Errors
///
/// 目录创建失败时返回错误描述字符串。
pub fn ensure_app_dir() -> Result<(), String> {
    let dir = get_app_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create app dir: {}", e))
}

/// 确保配置目录存在（当前与 `ensure_app_dir` 等效）。
///
/// # Errors
///
/// 目录创建失败时返回错误描述字符串。
pub fn ensure_config_dir() -> Result<(), String> {
    ensure_app_dir()
}

fn get_legacy_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".xechat")
}

/// 获取旧版（v1）配置文件路径。
///
/// # Returns
///
/// `~/.xechat/config.toml` 的绝对路径。
pub fn get_legacy_config_path() -> PathBuf {
    get_legacy_dir().join("config.toml")
}

/// 获取旧版（v1）对话数据文件路径。
///
/// # Returns
///
/// `~/.xechat/conversations.json` 的绝对路径。
pub fn get_legacy_conversations_path() -> PathBuf {
    get_legacy_dir().join("conversations.json")
}

/// 获取 LanceDB 统一数据存储路径。
pub fn get_lancedb_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("XEChat")
        .join("lancedb")
}
