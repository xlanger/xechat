use crate::XEChatConfig;
use std::fs;
use crate::services::paths;

#[cfg_attr(test, mockall::automock)]
pub trait ConfigService: Send + Sync {
    fn load_config(&self) -> Result<XEChatConfig, String>;
    fn save_config(&self, config: &XEChatConfig) -> Result<(), String>;
}

pub struct FileConfigService;

impl ConfigService for FileConfigService {
    fn load_config(&self) -> Result<XEChatConfig, String> {
        load_config()
    }
    fn save_config(&self, config: &XEChatConfig) -> Result<(), String> {
        save_config(config)
    }
}

/// 读取并解析配置文件。
///
/// 配置文件存在时读取并解析，不存在或解析失败时返回 `None`。
pub fn read_config_file(config_path: &std::path::Path) -> Option<XEChatConfig> {
    if !config_path.exists() {
        return None;
    }
    let content = fs::read_to_string(config_path).ok()?;
    toml::from_str(&content).ok()
}

/// 从磁盘加载原始配置。
///
/// 若配置文件存在，则读取并解析为 [`XEChatConfig`]；若不存在，
/// 则生成默认配置并写入磁盘后返回。
///
/// # Returns
///
/// 成功返回解析后的配置对象。
///
/// # Errors
///
/// 文件读取失败或 TOML 解析失败时返回错误描述字符串。
pub fn load_config_raw() -> Result<XEChatConfig, String> {
    paths::ensure_config_dir()?;
    let config_path = paths::get_config_path();

    if let Some(config) = read_config_file(&config_path) {
        Ok(config)
    } else {
        let default_config = XEChatConfig::default();
        save_config(&default_config)?;
        Ok(default_config)
    }
}

/// 将配置持久化到磁盘。
///
/// 以 TOML 格式将 [`XEChatConfig`] 写入默认配置路径。
///
/// # Arguments
///
/// * `config` - 待保存的配置对象引用
///
/// # Errors
///
/// TOML 序列化失败或文件写入失败时返回错误描述字符串。
pub fn save_config(config: &XEChatConfig) -> Result<(), String> {
    paths::ensure_config_dir()?;
    let config_path = paths::get_config_path();
    let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&config_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// 加载配置。
///
/// 调用 [`load_config_raw`] 返回配置，并与默认配置合并以确保
/// 新版本新增的 provider 和字段不会因旧配置文件缺失而丢失。
///
/// # Returns
///
/// 成功返回 [`XEChatConfig`]。
///
/// # Errors
///
/// 文件读取或 TOML 解析失败时返回错误描述字符串。
pub fn load_config() -> Result<XEChatConfig, String> {
    let mut config = load_config_raw()?;
    let default = XEChatConfig::default();

    // 补全默认 provider：旧配置文件可能缺少新版本新增的 provider
    for (key, default_provider) in default.model_providers {
        if !config.model_providers.contains_key(&key) {
            config.model_providers.insert(key, default_provider);
        }
    }

    Ok(config)
}
