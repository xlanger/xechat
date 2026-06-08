//! Windows 平台特定实现

use dioxus::desktop::{Config, WindowBuilder};
use super::SystemTheme;

/// 检测 Windows 系统语言，返回 locale 标识符。
/// 检测失败或不支持时返回 "zh-CN"。
pub fn detect_system_language() -> &'static str {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

    unsafe {
        let mut buf = [0u16; 85];
        let len = GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32);
        if len > 0 {
            let locale = OsString::from_wide(&buf[..len as usize - 1])
                .to_string_lossy()
                .to_lowercase();
            if locale.starts_with("zh") {
                return "zh-CN";
            } else if locale.starts_with("en") {
                return "en";
            }
        }
    }
    "zh-CN"
}

/// 配置 Windows 特定的窗口选项
pub fn configure_window(window_builder: WindowBuilder) -> WindowBuilder {
    // Windows 默认配置，无需特殊设置
    window_builder
}

/// 获取 Windows 特定的应用配置
pub fn app_config() -> Config {
    Config::new()
}

/// 检测 Windows 系统的主题。
///
/// 通过读取注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// 下的 `AppsUseLightTheme` 值判断系统主题。
/// 值为 1 表示浅色模式，0 表示深色模式。
/// 读取失败时默认返回深色模式。
pub fn detect_system_theme() -> SystemTheme {
    use std::process::Command;

    // 通过 reg query 读取注册表值
    if let Ok(output) = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // 输出格式如：AppsUseLightTheme    REG_DWORD    0x1
            if stdout.contains("0x1") {
                return SystemTheme::Light;
            } else if stdout.contains("0x0") {
                return SystemTheme::Dark;
            }
        }
    }

    SystemTheme::Dark
}
