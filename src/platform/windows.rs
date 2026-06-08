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

/// 检测 Windows 系统的主题
pub fn detect_system_theme() -> SystemTheme {
    // Windows 主题检测实现
    // TODO: 实际实现可通过注册表或其他方式
    SystemTheme::Dark
}
