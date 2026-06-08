//! 应用全局状态管理 Store。
//!
//! 持有应用配置、主题模式和当前语言等全局状态。

use dioxus::prelude::*;
use crate::{XEChatConfig};
use crate::services::config;
use crate::models::i18n::{Language, set_language};
use crate::state::{ThemeMode, MainRoute};

/// 应用全局状态 Store，管理配置、主题、语言、时区和路由。
#[derive(Copy, Clone)]
pub struct AppStore {
    /// 应用配置（从文件加载，可选）
    pub config: Signal<Option<XEChatConfig>>,
    /// 当前主题模式（Light / Dark / System）
    pub theme_mode: Signal<ThemeMode>,
    /// 当前语言
    pub language: Signal<Language>,
    /// 当前时区（IANA 标识符，如 `"Asia/Shanghai"`，`"system"` 表示本地时区）
    pub timezone: Signal<String>,
    /// 当前主内容区路由
    pub main_route: Signal<MainRoute>,
}

impl Default for AppStore {
    fn default() -> Self {
        Self::new()
    }
}

impl AppStore {
    /// 创建 AppStore 实例并初始化所有信号为默认值。
    pub fn new() -> Self {
        let lang = Language::Zh;
        set_language(lang);
        Self {
            config: Signal::new(None),
            theme_mode: Signal::new(ThemeMode::System),
            language: Signal::new(lang),
            timezone: Signal::new("system".to_string()),
            main_route: Signal::new(MainRoute::Welcome),
        }
    }

    /// 从持久化存储加载应用配置，并同步 theme_mode 和 language signal。
    pub async fn load_config(&mut self) {
        if let Ok(c) = config::load_config() {
            // 同步 theme_mode
            let theme_mode = match c.theme.as_str() {
                "dark" => ThemeMode::Dark,
                "light" => ThemeMode::Light,
                _ => ThemeMode::System,
            };
            self.theme_mode.set(theme_mode);

            // 同步 language：system 或无法匹配时回退到中文
            let lang = match c.language.as_str() {
                "system" => Language::System,
                "en" => Language::En,
                _ => Language::Zh,
            };
            self.language.set(lang);
            set_language(lang);

            // 同步 timezone
            let tz = if c.timezone.is_empty() || c.timezone == "system" {
                "system".to_string()
            } else {
                c.timezone.clone()
            };
            self.timezone.set(tz);

            self.config.set(Some(c));
        }
    }

    /// 更新配置并持久化到磁盘。
    ///
    /// 接受一个闭包来修改配置，修改后会自动保存到 TOML 文件。
    /// 同时同步 theme_mode 和 language signal。
    pub fn update_config<F>(&mut self, f: F)
    where
        F: FnOnce(&mut XEChatConfig),
    {
        let mut config_opt = None;
        {
            let config_ref = self.config.read();
            if let Some(config) = config_ref.as_ref() {
                let mut config = config.clone();
                f(&mut config);
                config_opt = Some(config);
            }
        }

        if let Some(config) = config_opt {
            // 同步 theme_mode signal
            let theme_mode = match config.theme.as_str() {
                "dark" => ThemeMode::Dark,
                "light" => ThemeMode::Light,
                _ => ThemeMode::System,
            };
            self.theme_mode.set(theme_mode);

            // 同步 language signal：system 或无法匹配时回退到中文
            let lang = match config.language.as_str() {
                "system" => Language::System,
                "en" => Language::En,
                _ => Language::Zh,
            };
            self.language.set(lang);
            set_language(lang);

            // 保存到磁盘
            let _ = config::save_config(&config);

            // 更新 config signal
            self.config.set(Some(config));
        }
    }

    /// 设置当前主题模式，同时同步到 config 并保存。
    pub fn set_theme_mode(&mut self, theme_mode: ThemeMode) {
        let theme_str = match theme_mode {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
            ThemeMode::System => "system",
        };
        self.update_config(|config| {
            config.theme = theme_str.to_string();
        });
        self.theme_mode.set(theme_mode);
    }

    /// 设置当前语言，同时同步到 config 并保存。
    pub fn set_language(&mut self, lang: Language) {
        let lang_str = match lang {
            Language::System => "system",
            Language::Zh => "zh",
            Language::En => "en",
        };
        self.update_config(|config| {
            config.language = lang_str.to_string();
        });
        self.language.set(lang);
        set_language(lang);
    }

    /// 设置当前时区，同时同步到 config 并保存。
    pub fn set_timezone(&mut self, tz: String) {
        self.update_config(|config| {
            config.timezone = tz.clone();
        });
        self.timezone.set(tz);
    }

    /// 导航到指定路由。
    ///
    /// 更新 main_route signal，统一控制 main_content 区域显示内容。
    pub fn navigate_to(&mut self, route: MainRoute) {
        self.main_route.set(route);
    }
}
