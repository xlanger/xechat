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
    /// 网络是否可用
    pub network_available: Signal<bool>,
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
            network_available: Signal::new(true),
        }
    }

    /// 从主题字符串解析 ThemeMode。
    pub fn parse_theme_mode(theme: &str) -> ThemeMode {
        match theme {
            "dark" => ThemeMode::Dark,
            "light" => ThemeMode::Light,
            _ => ThemeMode::System,
        }
    }

    /// 从语言字符串解析 Language。
    pub fn parse_language(lang: &str) -> Language {
        match lang {
            "system" => Language::System,
            "en" => Language::En,
            _ => Language::Zh,
        }
    }

    /// 规范化时区字符串：空或 "system" 统一为 "system"。
    pub fn normalize_timezone(tz: &str) -> String {
        if tz.is_empty() || tz == "system" {
            "system".to_string()
        } else {
            tz.to_string()
        }
    }

    /// 从持久化存储加载应用配置，并同步 theme_mode 和 language signal。
    pub async fn load_config(&mut self) {
        if let Ok(c) = config::load_config() {
            self.theme_mode.set(Self::parse_theme_mode(&c.theme));
            let lang = Self::parse_language(&c.language);
            self.language.set(lang);
            set_language(lang);
            self.timezone.set(Self::normalize_timezone(&c.timezone));
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
            self.theme_mode.set(Self::parse_theme_mode(&config.theme));
            let lang = Self::parse_language(&config.language);
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

    /// 从配置中提取当前提供商的 base_url。
    pub fn resolve_primary_url(config: &XEChatConfig) -> Option<String> {
        let primary_url = config
            .model_providers
            .get(&config.model_provider)
            .and_then(|p| {
                let url = p.base_url.trim();
                if url.is_empty() { None } else { Some(url.to_string()) }
            });

        if config.model_provider == "ollama" {
            let host = if config.preferences.ollama.host.is_empty() {
                "http://localhost:11434"
            } else {
                &config.preferences.ollama.host
            };
            Some(host.to_string())
        } else {
            primary_url
        }
    }

    /// 判断 URL 是否为本地服务地址。
    pub fn is_local_url(url: &str) -> bool {
        url.starts_with("http://localhost")
            || url.starts_with("http://127.0.0.1")
            || url.starts_with("http://[::1]")
    }

    /// 异步检测网络连通性并更新 `network_available` 信号。
    ///
    /// 检测策略（优先级从高到低）：
    /// 1. **主检测**：当前对话模型提供商的 base_url（HEAD 请求）
    /// 2. **辅检测**：公共端点（github.com），作为地域 DNS fallback
    /// 3. **本地服务**（localhost / 127.0.0.1 / ::1）：跳过检测，视为可用
    pub async fn check_network(&mut self) {
        let config = crate::services::config::load_config().unwrap_or_default();
        let primary_url = Self::resolve_primary_url(&config);

        if primary_url.as_deref().map_or(false, Self::is_local_url) {
            self.network_available.set(true);
            return;
        }

        let Ok(client) = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .connect_timeout(std::time::Duration::from_secs(3))
            .build()
        else {
            self.network_available.set(false);
            return;
        };

        // 主检测：对话模型 host
        if let Some(ref url) = primary_url {
            let target = url.trim_end_matches('/');
            if client.head(target).send().await.is_ok() {
                self.network_available.set(true);
                return;
            }
        }

        // 辅检测：公共端点（地域 DNS fallback）
        let fallback_ok = client.head("https://github.com").send().await.is_ok();
        self.network_available.set(fallback_ok);
    }
}
