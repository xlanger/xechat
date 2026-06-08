//! macOS 平台特定实现

use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;
use dioxus::desktop::{Config, WindowBuilder};
use super::SystemTheme;

/// 检测 macOS 系统语言，返回 locale 标识符。
/// 检测失败或不支持时返回 "zh-CN"。
pub fn detect_system_language() -> &'static str {
    use std::process::Command;
    if let Ok(output) = Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output()
        && output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // 输出格式如：(en, zh-Hans, ja)
            if let Some(lang) = stdout.split(',').next() {
                let lang = lang.trim().trim_start_matches('(').trim_matches('"');
                if lang.starts_with("zh") {
                    return "zh-CN";
                } else if lang.starts_with("en") {
                    return "en";
                }
            }
        }
    "zh-CN"
}

/// 配置 macOS 特定的窗口选项
pub fn configure_window(window_builder: WindowBuilder) -> WindowBuilder {
    window_builder
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
}

/// 获取 macOS 特定的应用配置
pub fn app_config() -> Config {
    Config::new()
}

/// 检测 macOS 系统的主题
pub fn detect_system_theme() -> SystemTheme {
    // 尝试通过 macOS defaults 读取系统主题设置
    use std::process::Command;
    
    if let Ok(output) = Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        && let Ok(stdout) = String::from_utf8(output.stdout)
            && stdout.trim().to_lowercase().contains("dark") {
                return SystemTheme::Dark;
            }
    
    // 如果读取失败或者值不存在，默认为浅色模式
    SystemTheme::Light
}
