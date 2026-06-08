//! 设置页面 - 通用设置区段。
//!
//! 提供主题模式、语言、时区选择器，以及嵌入模型和对话模型的切换下拉框。
//! 修改实时同步到全局 config 并自动持久化。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_app;
use crate::components::custom_select::CustomSelect;

fn get_theme(config: &Option<crate::models::config::XEChatConfig>) -> String {
    config
        .as_ref()
        .map(|c| c.theme.clone())
        .unwrap_or_else(|| "system".to_string())
}

fn get_language(config: &Option<crate::models::config::XEChatConfig>) -> String {
    config
        .as_ref()
        .map(|c| c.language.clone())
        .unwrap_or_else(|| "zh".to_string())
}

fn get_timezone(config: &Option<crate::models::config::XEChatConfig>) -> String {
    config
        .as_ref()
        .map(|c| c.timezone.clone())
        .unwrap_or_else(|| "system".to_string())
}

fn get_embed_provider(config: &Option<crate::models::config::XEChatConfig>) -> String {
    config.as_ref().map(|c| c.preferences.embed_provider.clone()).unwrap_or_else(|| "default".to_string())
}

/// 常用时区列表（IANA 标识符 + 显示名）
fn timezone_options() -> Vec<(String, String)> {
    vec![
        ("system".into(), t!("settings.tz-system").into()),
        ("Asia/Shanghai".into(), "Asia/Shanghai (UTC+8)".into()),
        ("Asia/Tokyo".into(), "Asia/Tokyo (UTC+9)".into()),
        ("Asia/Kolkata".into(), "Asia/Kolkata (UTC+5:30)".into()),
        ("Asia/Dubai".into(), "Asia/Dubai (UTC+4)".into()),
        ("Europe/London".into(), "Europe/London (UTC+0/+1)".into()),
        ("Europe/Berlin".into(), "Europe/Berlin (UTC+1/+2)".into()),
        ("Europe/Moscow".into(), "Europe/Moscow (UTC+3)".into()),
        ("America/New_York".into(), "America/New_York (UTC-5/-4)".into()),
        ("America/Chicago".into(), "America/Chicago (UTC-6/-5)".into()),
        ("America/Denver".into(), "America/Denver (UTC-7/-6)".into()),
        ("America/Los_Angeles".into(), "America/Los_Angeles (UTC-8/-7)".into()),
        ("Pacific/Auckland".into(), "Pacific/Auckland (UTC+12/+13)".into()),
    ]
}

#[with_css(css, "styles/components/settings.scss")]
#[component]
pub fn GeneralSection() -> Element {
    let mut app_store = use_app();

    // Ollama 探测结果缓存
    let mut ollama_models: Signal<Vec<String>> = use_signal(Vec::new);
    let mut ollama_embed_models: Signal<Vec<String>> = use_signal(Vec::new);

    let general_text = t!("settings.general").to_string();
    let theme_text = t!("settings.theme").to_string();
    let language_text = t!("settings.language").to_string();
    let timezone_text = t!("settings.timezone").to_string();
    let embed_provider_text = t!("settings.embed-provider").to_string();
    let embed_model_text = t!("settings.current-embed-model").to_string();
    let chat_provider_text = t!("settings.chat-provider").to_string();
    let chat_model_text = t!("settings.chat-model").to_string();

    let theme_options = vec![
        ("system".into(), t!("settings.theme-system").into()),
        ("dark".into(), t!("settings.theme-dark").into()),
        ("light".into(), t!("settings.theme-light").into()),
    ];

    let lang_options = vec![
        ("system".into(), t!("settings.lang-system").into()),
        ("zh".into(), t!("settings.lang-zh").into()),
        ("en".into(), t!("settings.lang-en").into()),
    ];

    let tz_options = timezone_options();

    let embed_provider_options = vec![
        ("default".to_string(), t!("settings.ollama-embed-provider-default").to_string()),
        ("ollama".to_string(), t!("settings.ollama-embed-provider-ollama").to_string()),
    ];

    let (provider_options, current_provider, current_model) = {
        let config_guard = app_store.config.read();
        match config_guard.as_ref() {
            Some(config) => {
                let provider_opts: Vec<(String, String)> = config.model_providers.iter()
                    .map(|(k, v)| (k.clone(), v.name.clone()))
                    .collect();
                let cur_provider = config.model_provider.clone();
                let cur_model = config.model.clone();
                (provider_opts, cur_provider, cur_model)
            }
            None => (Vec::new(), String::new(), String::new()),
        }
    };

    let current_embed_provider = get_embed_provider(&app_store.config.read());

    // 初始加载时探测 Ollama 模型
    {
        let is_ollama = current_provider == "ollama";
        let is_ollama_embed = current_embed_provider == "ollama";
        use_effect(move || {
            if is_ollama {
                let host = app_store.config.read().as_ref()
                    .map(|c| c.preferences.ollama.host.clone())
                    .unwrap_or_default();
                if !host.is_empty() {
                    spawn(async move {
                        let models = crate::services::ollama::probe::fetch_chat_models(&host).await;
                        ollama_models.set(models);
                    });
                }
            }
            if is_ollama_embed {
                let host = app_store.config.read().as_ref()
                    .map(|c| c.preferences.ollama.host.clone())
                    .unwrap_or_default();
                if !host.is_empty() {
                    spawn(async move {
                        let models = crate::services::ollama::probe::fetch_embed_models(&host).await;
                        ollama_embed_models.set(models);
                    });
                }
            }
        });
    }

    // 切换 Provider 时触发 Ollama 探测
    let on_select_provider = {
        let mut ollama_models = ollama_models;
        move |v: String| {
            let is_ollama = v == "ollama";
            app_store.update_config(|config| {
                config.model_provider = v.clone();
                if let Some(provider) = config.model_providers.get(&v) {
                    if let Some(first_model) = provider.models.keys().next() {
                        config.model = first_model.clone();
                    } else {
                        config.model = String::new();
                    }
                }
            });
            if is_ollama {
                let host = app_store.config.read().as_ref()
                    .map(|c| c.preferences.ollama.host.clone())
                    .unwrap_or_default();
                if !host.is_empty() {
                    spawn(async move {
                        let chat = crate::services::ollama::probe::fetch_chat_models(&host).await;
                        ollama_models.set(chat);
                    });
                }
            } else {
                ollama_models.set(Vec::new());
            }
        }
    };

    // 切换嵌入提供商时触发 Ollama 探测
    let on_select_embed_provider = {
        let mut ollama_embed_models = ollama_embed_models;
        move |v: String| {
            let is_ollama = v == "ollama";
            app_store.update_config(|config| {
                config.preferences.embed_provider = v.clone();
                if config.preferences.embed_provider != "ollama" {
                    config.preferences.ollama.embed_model = String::new();
                }
            });
            if is_ollama {
                let host = app_store.config.read().as_ref()
                    .map(|c| c.preferences.ollama.host.clone())
                    .unwrap_or_default();
                if !host.is_empty() {
                    spawn(async move {
                        let embed = crate::services::ollama::probe::fetch_embed_models(&host).await;
                        ollama_embed_models.set(embed);
                    });
                }
            } else {
                ollama_embed_models.set(Vec::new());
            }
        }
    };

    rsx! {
        section {
            class: "{css::settings_section}",
            h3 {
                class: "{css::section_title}",
                "{general_text}"
            }
            div {
                class: "{css::form_row_inline}",
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        "{theme_text}"
                    }
                    CustomSelect {
                        options: theme_options.clone(),
                        value: get_theme(&app_store.config.read()),
                        on_select: move |v: String| {
                            let mode = match v.as_str() {
                                "dark" => crate::state::ThemeMode::Dark,
                                "light" => crate::state::ThemeMode::Light,
                                _ => crate::state::ThemeMode::System,
                            };
                            app_store.set_theme_mode(mode);
                        },
                    }
                }
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        "{language_text}"
                    }
                    CustomSelect {
                        options: lang_options.clone(),
                        value: get_language(&app_store.config.read()),
                        on_select: move |v: String| {
                            let lang = match v.as_str() {
                                "system" => crate::models::i18n::Language::System,
                                "en" => crate::models::i18n::Language::En,
                                _ => crate::models::i18n::Language::Zh,
                            };
                            app_store.set_language(lang);
                        },
                    }
                }
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        "{timezone_text}"
                    }
                    CustomSelect {
                        options: tz_options,
                        value: get_timezone(&app_store.config.read()),
                        on_select: move |v: String| {
                            app_store.set_timezone(v);
                        },
                    }
                }
            }
            div {
                class: "{css::form_row_inline}",
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        "{embed_provider_text}"
                    }
                    CustomSelect {
                        options: embed_provider_options.clone(),
                        value: current_embed_provider.clone(),
                        on_select: on_select_embed_provider,
                    }
                }
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        "{embed_model_text}"
                    }
                    {
                        let embed_model_opts = if current_embed_provider == "ollama" {
                            let mut opts: Vec<(String, String)> = vec![];
                            let detected = ollama_embed_models.read();
                            for name in detected.iter() {
                                opts.push((name.clone(), name.clone()));
                            }
                            if opts.is_empty() {
                                vec![("".to_string(), t!("settings.no-models").to_string())]
                            } else {
                                opts
                            }
                        } else {
                            vec![("default".to_string(), "E5".to_string())]
                        };
                        let current_embed_model = if current_embed_provider == "ollama" {
                            app_store.config.read().as_ref()
                                .map(|c| c.preferences.ollama.embed_model.clone())
                                .unwrap_or_default()
                        } else {
                            "default".to_string()
                        };
                        rsx! {
                            CustomSelect {
                                options: embed_model_opts,
                                value: current_embed_model,
                                on_select: move |v: String| {
                                    app_store.update_config(|config| {
                                        config.preferences.ollama.embed_model = v;
                                    });
                                },
                            }
                        }
                    }
                }
            }
            div {
                class: "{css::form_row_inline}",
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        "{chat_provider_text}"
                    }
                    CustomSelect {
                        options: provider_options.clone(),
                        value: current_provider.clone(),
                        on_select: on_select_provider,
                    }
                }
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        "{chat_model_text}"
                    }
                    {
                        let mut model_opts: Vec<(String, String)> = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&current_provider))
                            .map(|p| p.models.keys().map(|k| (k.clone(), k.clone())).collect())
                            .unwrap_or_default();
                        if current_provider == "ollama" {
                            let detected = ollama_models.read();
                            for name in detected.iter() {
                                if !model_opts.iter().any(|(k, _)| k == name) {
                                    model_opts.push((name.clone(), name.clone()));
                                }
                            }
                        }
                        rsx! {
                            CustomSelect {
                                options: model_opts,
                                value: current_model.clone(),
                                on_select: move |v: String| {
                                    app_store.update_config(|config| {
                                        config.model = v;
                                    });
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
