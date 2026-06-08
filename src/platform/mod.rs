//! 跨平台功能模块
//! 
//! 此模块封装了不同平台的特定实现，提供统一的 API。

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
pub use self::macos::*;
#[cfg(target_os = "windows")]
pub use self::windows::*;
#[cfg(target_os = "linux")]
pub use self::linux::*;

/// 平台名称
pub fn platform_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macOS"
    }
    #[cfg(target_os = "windows")]
    {
        "Windows"
    }
    #[cfg(target_os = "linux")]
    {
        "Linux"
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        "Unknown"
    }
}

/// 获取系统当前的主题模式
pub fn detect_system_theme() -> SystemTheme {
    #[cfg(target_os = "macos")]
    {
        macos::detect_system_theme()
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_system_theme();
    }
    #[cfg(target_os = "linux")]
    {
        return linux::detect_system_theme();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return SystemTheme::Dark;
    }
}

/// 检测系统语言，返回 locale 标识符。
/// 检测失败或不支持时回退到 "zh-CN"。
pub fn detect_system_language() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        macos::detect_system_language()
    }
    #[cfg(target_os = "windows")]
    {
        return windows::detect_system_language();
    }
    #[cfg(target_os = "linux")]
    {
        return linux::detect_system_language();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        return "zh-CN";
    }
}

/// 系统主题
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemTheme {
    /// 浅色模式
    Light,
    /// 深色模式
    Dark,
}

impl SystemTheme {
    /// 返回主题名称字符串（"light" 或 "dark"）
    pub fn as_str(&self) -> &'static str {
        match self {
            SystemTheme::Light => "light",
            SystemTheme::Dark => "dark",
        }
    }
}
