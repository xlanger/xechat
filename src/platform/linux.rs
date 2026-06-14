//! Linux 平台特定实现

use dioxus::desktop::{Config, WindowBuilder};
use super::SystemTheme;

/// 检测 Linux 系统语言，返回 locale 标识符。
/// 检测失败或不支持时返回 "zh-CN"。
pub fn detect_system_language() -> &'static str {
    if let Ok(locale) = std::env::var("LANG") {
        let locale_lower = locale.to_lowercase();
        if locale_lower.starts_with("en") {
            return "en";
        } else if locale_lower.starts_with("zh") {
            return "zh-CN";
        }
    }
    "zh-CN"
}

/// 配置 Linux 特定的窗口选项
pub fn configure_window(window_builder: WindowBuilder) -> WindowBuilder {
    // Linux 默认配置，无需特殊设置
    window_builder
}

/// 获取 Linux 特定的应用配置
pub fn app_config() -> Config {
    Config::new()
}

/// 检测 Linux 系统的主题。
///
/// 按优先级尝试以下方式：
/// 1. `XDG_CONFIG_HOME/gtk-4.0/settings.ini` 或 `~/.config/gtk-4.0/settings.ini` 中的 `gtk-application-prefer-dark-theme`
/// 2. `gsettings get org.gnome.desktop.interface color-scheme`（GNOME 42+）
/// 3. `gsettings get org.gnome.desktop.interface gtk-theme`（旧版 GNOME）
/// 4. 环境变量 `GTK_THEME`
/// 5. 默认回退到 Dark
pub fn detect_system_theme() -> SystemTheme {
    detect_from_gtk4_config()
        .or_else(detect_from_gnome_color_scheme)
        .or_else(detect_from_gnome_gtk_theme)
        .or_else(detect_from_env_var)
        .unwrap_or(SystemTheme::Dark)
}

/// 判断 GTK4 配置行是否包含深色主题值（`=true` 或 `=1`）。
#[inline]
pub fn is_dark_theme_value(line: &str) -> bool {
    line.contains("=true") || line.contains("=1")
}

/// 判断 GTK4 配置行是否包含浅色主题值（`=false` 或 `=0`）。
#[inline]
pub fn is_light_theme_value(line: &str) -> bool {
    line.contains("=false") || line.contains("=0")
}

/// 从 GTK4 配置行中解析 `gtk-application-prefer-dark-theme` 的值。
///
/// 支持的值格式：`=true`、`=1`（Dark）、`=false`、`=0`（Light）。
///
/// # Arguments
///
/// * `line` - 配置文件中的一行文本
///
/// # Returns
///
/// 若行包含 `gtk-application-prefer-dark-theme` 键且值可识别，返回 `Some(SystemTheme)`；
/// 否则返回 `None`。
pub fn parse_gtk4_theme_line(line: &str) -> Option<SystemTheme> {
    if !line.starts_with("gtk-application-prefer-dark-theme") {
        return None;
    }
    if is_dark_theme_value(line) {
        Some(SystemTheme::Dark)
    } else if is_light_theme_value(line) {
        Some(SystemTheme::Light)
    } else {
        None
    }
}

/// 从 GTK4 配置文件检测主题。
fn detect_from_gtk4_config() -> Option<SystemTheme> {
    let config_dir = dirs::config_dir()?;
    let gtk4_settings = config_dir.join("gtk-4.0/settings.ini");
    if !gtk4_settings.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&gtk4_settings).ok()?;
    content.lines().find_map(parse_gtk4_theme_line)
}

/// 从 GNOME 42+ color-scheme 设置检测主题。
fn detect_from_gnome_color_scheme() -> Option<SystemTheme> {
    detect_from_gsettings("color-scheme")
}

/// 从旧版 GNOME gtk-theme 设置检测主题。
fn detect_from_gnome_gtk_theme() -> Option<SystemTheme> {
    detect_from_gsettings("gtk-theme")
}

/// 通过 gsettings 查询指定键值，根据返回值判断深浅色主题。
///
/// 返回值中包含 "dark" 返回 Dark，包含 "light" 返回 Light，否则返回 None。
fn detect_from_gsettings(key: &str) -> Option<SystemTheme> {
    use std::process::Command;

    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_theme_from_string(&stdout)
}

/// 从字符串中解析主题关键词。
///
/// 包含 "dark" 返回 Dark，包含 "light" 返回 Light，否则返回 None。
pub fn parse_theme_from_string(s: &str) -> Option<SystemTheme> {
    let lower = s.to_lowercase();
    if lower.contains("dark") {
        Some(SystemTheme::Dark)
    } else if lower.contains("light") {
        Some(SystemTheme::Light)
    } else {
        None
    }
}

/// 从 GTK_THEME 环境变量检测主题。
fn detect_from_env_var() -> Option<SystemTheme> {
    let gtk_theme = std::env::var("GTK_THEME").ok()?;
    parse_theme_from_string(&gtk_theme)
}
