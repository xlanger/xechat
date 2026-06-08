//! 国际化（i18n）支持模块，基于 rust_i18n。
//!
//! 使用方法：
//! - 在需要翻译的地方导入 `use rust_i18n::t;`
//! - 使用 `t!("key")` 获取翻译文本
//! - 使用 `t!("key", name = "world")` 带参数翻译
//! - 使用 `rust_i18n::set_locale("zh-CN")` 切换语言

use crate::platform;

/// 应用支持的语言类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// 跟随系统语言
    System,
    /// 简体中文
    Zh,
    /// English（英语）
    En,
}

impl Language {
    /// 获取语言对应的 locale 标识符。
    /// System 时检测系统语言，无法检测或匹配失败时回退到中文。
    pub fn to_locale(&self) -> &'static str {
        match self {
            Language::System => platform::detect_system_language(),
            Language::Zh => "zh-CN",
            Language::En => "en",
        }
    }

    /// 从 locale 标识符解析语言。
    pub fn from_locale(locale: &str) -> Option<Self> {
        match locale {
            "zh" | "zh-CN" | "zh-TW" => Some(Language::Zh),
            "en" | "en-US" | "en-GB" => Some(Language::En),
            _ => None,
        }
    }
}

/// 设置全局语言。
pub fn set_language(lang: Language) {
    rust_i18n::set_locale(lang.to_locale());
}
