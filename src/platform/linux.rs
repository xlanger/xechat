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
    use std::process::Command;

    // 1. 尝试读取 GTK4 配置文件
    if let Some(config_dir) = dirs::config_dir() {
        let gtk4_settings = config_dir.join("gtk-4.0/settings.ini");
        if gtk4_settings.exists() {
            if let Ok(content) = std::fs::read_to_string(&gtk4_settings) {
                for line in content.lines() {
                    if line.starts_with("gtk-application-prefer-dark-theme") {
                        if line.contains("=true") || line.contains("=1") {
                            return SystemTheme::Dark;
                        } else if line.contains("=false") || line.contains("=0") {
                            return SystemTheme::Light;
                        }
                    }
                }
            }
        }
    }

    // 2. 尝试 GNOME 42+ color-scheme
    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.to_lowercase().contains("dark") {
                return SystemTheme::Dark;
            } else if stdout.to_lowercase().contains("light") {
                return SystemTheme::Light;
            }
        }
    }

    // 3. 尝试旧版 GNOME gtk-theme
    if let Ok(output) = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let theme = stdout.to_lowercase();
            if theme.contains("dark") {
                return SystemTheme::Dark;
            } else if theme.contains("light") {
                return SystemTheme::Light;
            }
        }
    }

    // 4. 尝试 GTK_THEME 环境变量
    if let Ok(gtk_theme) = std::env::var("GTK_THEME") {
        let theme = gtk_theme.to_lowercase();
        if theme.contains("dark") {
            return SystemTheme::Dark;
        } else if theme.contains("light") {
            return SystemTheme::Light;
        }
    }

    // 5. 默认回退
    SystemTheme::Dark
}
