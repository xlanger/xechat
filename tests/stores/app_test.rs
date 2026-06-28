#[path = "../common/mod.rs"]
mod common;

use dioxus::prelude::*;
use dioxus_core::{NoOpMutations, Runtime, RuntimeGuard};
use xechat::stores::app::AppStore;
use xechat::state::{MainRoute, ThemeMode};
use xechat::models::i18n::Language;
use xechat::XEChatConfig;

fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let mut vdom = VirtualDom::new(|| rsx! { div {} });
    vdom.rebuild(&mut NoOpMutations);
    vdom.in_runtime(|| {
        let runtime = Runtime::current();
        let _guard = RuntimeGuard::new(runtime.clone());
        runtime.in_scope(ScopeId::APP, f)
    })
}

#[test]
fn test_app_store_new_default_values() {
    with_runtime(|| {
        let store = AppStore::new();

        assert!((store.config)().is_none());
        assert_eq!((store.theme_mode)(), ThemeMode::System);
        assert_eq!((store.language)(), Language::Zh);
        assert_eq!((store.main_route)(), MainRoute::Welcome);
    });
}

#[test]
fn test_update_config_syncs_theme_and_language() {
    let _guard = common::setup_temp_dir();

    with_runtime(|| {
        let mut store = AppStore::new();
        store.config.set(Some(XEChatConfig::default()));

        store.update_config(|config| {
            config.theme = "dark".to_string();
            config.language = "en".to_string();
        });

        assert_eq!((store.theme_mode)(), ThemeMode::Dark);
        assert_eq!((store.language)(), Language::En);

        let config = (store.config)().expect("config should be set");
        assert_eq!(config.theme, "dark");
        assert_eq!(config.language, "en");
    });
}

#[test]
fn test_update_config_light_theme() {
    let _guard = common::setup_temp_dir();

    with_runtime(|| {
        let mut store = AppStore::new();
        store.config.set(Some(XEChatConfig::default()));

        store.update_config(|config| {
            config.theme = "light".to_string();
        });

        assert_eq!((store.theme_mode)(), ThemeMode::Light);
    });
}

#[test]
fn test_update_config_system_language() {
    let _guard = common::setup_temp_dir();

    with_runtime(|| {
        let mut store = AppStore::new();
        store.config.set(Some(XEChatConfig::default()));

        store.update_config(|config| {
            config.language = "system".to_string();
        });

        assert_eq!((store.language)(), Language::System);
    });
}

#[test]
fn test_update_config_noop_when_config_none() {
    with_runtime(|| {
        let mut store = AppStore::new();
        assert!((store.config)().is_none());

        store.update_config(|config| {
            config.theme = "dark".to_string();
        });

        assert!((store.config)().is_none());
        assert_eq!((store.theme_mode)(), ThemeMode::System);
    });
}

#[test]
fn test_set_theme_mode_updates_signal_and_config() {
    let _guard = common::setup_temp_dir();

    with_runtime(|| {
        let mut store = AppStore::new();
        store.config.set(Some(XEChatConfig::default()));

        store.set_theme_mode(ThemeMode::Dark);

        assert_eq!((store.theme_mode)(), ThemeMode::Dark);
        let config = (store.config)().expect("config should be set");
        assert_eq!(config.theme, "dark");
    });
}

#[test]
fn test_set_theme_mode_light() {
    let _guard = common::setup_temp_dir();

    with_runtime(|| {
        let mut store = AppStore::new();
        store.config.set(Some(XEChatConfig::default()));

        store.set_theme_mode(ThemeMode::Light);

        assert_eq!((store.theme_mode)(), ThemeMode::Light);
        let config = (store.config)().expect("config should be set");
        assert_eq!(config.theme, "light");
    });
}

#[test]
fn test_set_theme_mode_signal_updated_even_without_config() {
    with_runtime(|| {
        let mut store = AppStore::new();
        assert!((store.config)().is_none());

        store.set_theme_mode(ThemeMode::Dark);

        assert_eq!((store.theme_mode)(), ThemeMode::Dark);
        assert!((store.config)().is_none());
    });
}

#[test]
fn test_set_language_updates_signal_and_config() {
    let _guard = common::setup_temp_dir();

    with_runtime(|| {
        let mut store = AppStore::new();
        store.config.set(Some(XEChatConfig::default()));

        store.set_language(Language::En);

        assert_eq!((store.language)(), Language::En);
        let config = (store.config)().expect("config should be set");
        assert_eq!(config.language, "en");
    });
}

#[test]
fn test_set_language_system() {
    let _guard = common::setup_temp_dir();

    with_runtime(|| {
        let mut store = AppStore::new();
        store.config.set(Some(XEChatConfig::default()));

        store.set_language(Language::System);

        assert_eq!((store.language)(), Language::System);
        let config = (store.config)().expect("config should be set");
        assert_eq!(config.language, "system");
    });
}

#[test]
fn test_set_language_signal_updated_even_without_config() {
    with_runtime(|| {
        let mut store = AppStore::new();
        assert!((store.config)().is_none());

        store.set_language(Language::En);

        assert_eq!((store.language)(), Language::En);
        assert!((store.config)().is_none());
    });
}

#[test]
fn test_navigate_to_changes_main_route() {
    with_runtime(|| {
        let mut store = AppStore::new();
        assert_eq!((store.main_route)(), MainRoute::Welcome);

        store.navigate_to(MainRoute::Settings);
        assert_eq!((store.main_route)(), MainRoute::Settings);
    });
}

#[test]
fn test_navigate_to_conversation() {
    with_runtime(|| {
        let mut store = AppStore::new();

        store.navigate_to(MainRoute::Conversation("conv-123".to_string()));
        assert_eq!(
            (store.main_route)(),
            MainRoute::Conversation("conv-123".to_string())
        );
    });
}

#[test]
fn test_navigate_to_welcome_from_settings() {
    with_runtime(|| {
        let mut store = AppStore::new();

        store.navigate_to(MainRoute::Settings);
        assert_eq!((store.main_route)(), MainRoute::Settings);

        store.navigate_to(MainRoute::Welcome);
        assert_eq!((store.main_route)(), MainRoute::Welcome);
    });
}

// ── parse_theme_mode, parse_language, normalize_timezone, is_local_url, resolve_primary_url ──

#[test]
fn test_parse_theme_mode_dark() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_theme_mode("dark"), ThemeMode::Dark);
    });
}

#[test]
fn test_parse_theme_mode_light() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_theme_mode("light"), ThemeMode::Light);
    });
}

#[test]
fn test_parse_theme_mode_system() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_theme_mode("system"), ThemeMode::System);
    });
}

#[test]
fn test_parse_theme_mode_unknown() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_theme_mode("unknown"), ThemeMode::System);
    });
}

#[test]
fn test_parse_theme_mode_empty() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_theme_mode(""), ThemeMode::System);
    });
}

#[test]
fn test_parse_language_en() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_language("en"), Language::En);
    });
}

#[test]
fn test_parse_language_system() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_language("system"), Language::System);
    });
}

#[test]
fn test_parse_language_zh() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_language("zh"), Language::Zh);
    });
}

#[test]
fn test_parse_language_unknown_defaults_zh() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_language("fr"), Language::Zh);
    });
}

#[test]
fn test_parse_language_empty_defaults_zh() {
    with_runtime(|| {
        assert_eq!(AppStore::parse_language(""), Language::Zh);
    });
}

#[test]
fn test_normalize_timezone_system() {
    with_runtime(|| {
        assert_eq!(AppStore::normalize_timezone("system"), "system");
    });
}

#[test]
fn test_normalize_timezone_empty() {
    with_runtime(|| {
        assert_eq!(AppStore::normalize_timezone(""), "system");
    });
}

#[test]
fn test_normalize_timezone_iana() {
    with_runtime(|| {
        assert_eq!(AppStore::normalize_timezone("Asia/Shanghai"), "Asia/Shanghai");
    });
}

#[test]
fn test_normalize_timezone_utc() {
    with_runtime(|| {
        assert_eq!(AppStore::normalize_timezone("UTC"), "UTC");
    });
}

#[test]
fn test_is_local_url_localhost() {
    with_runtime(|| {
        assert!(AppStore::is_local_url("http://localhost:8080"));
    });
}

#[test]
fn test_is_local_url_localhost_no_port() {
    with_runtime(|| {
        assert!(AppStore::is_local_url("http://localhost"));
    });
}

#[test]
fn test_is_local_url_127() {
    with_runtime(|| {
        assert!(AppStore::is_local_url("http://127.0.0.1:11434"));
    });
}

#[test]
fn test_is_local_url_ipv6_loopback() {
    with_runtime(|| {
        assert!(AppStore::is_local_url("http://[::1]:8080"));
    });
}

#[test]
fn test_is_local_url_remote() {
    with_runtime(|| {
        assert!(!AppStore::is_local_url("https://api.openai.com"));
    });
}

#[test]
fn test_is_local_url_https_localhost() {
    with_runtime(|| {
        assert!(!AppStore::is_local_url("https://localhost:8080"));
    });
}

#[test]
fn test_resolve_primary_url_ollama_default() {
    with_runtime(|| {
        let config = XEChatConfig::default();
        // Default provider may not be ollama, but we can test with ollama provider
        let mut config = config;
        config.model_provider = "ollama".to_string();
        let url = AppStore::resolve_primary_url(&config);
        assert_eq!(url, Some("http://localhost:11434".to_string()));
    });
}

#[test]
fn test_resolve_primary_url_ollama_custom_host() {
    with_runtime(|| {
        let mut config = XEChatConfig {
            model_provider: "ollama".to_string(),
            ..Default::default()
        };
        config.preferences.ollama.host = "http://192.168.1.100:11434".to_string();
        let url = AppStore::resolve_primary_url(&config);
        assert_eq!(url, Some("http://192.168.1.100:11434".to_string()));
    });
}

#[test]
fn test_resolve_primary_url_no_provider() {
    with_runtime(|| {
        let config = XEChatConfig::default();
        let url = AppStore::resolve_primary_url(&config);
        // Depends on default config, just verify it doesn't panic
        // It should return None or Some depending on config
        let _ = url;
    });
}
