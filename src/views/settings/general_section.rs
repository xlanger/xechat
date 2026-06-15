//! 设置页面 - 通用设置区段。
//!
//! 提供主题模式、语言、时区选择器，以及嵌入模型和对话模型的切换下拉框。
//! 修改实时同步到全局 config 并自动持久化。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::icons::{Icon, tabler};
use crate::hooks::use_app;
use crate::components::custom_select::CustomSelect;
use crate::components::tooltip::Tooltip;

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
    let mut ui_store = crate::hooks::use_ui();
    let mut conv_store = crate::hooks::use_conversation();

    // Ollama 探测结果缓存
    let mut ollama_models: Signal<Vec<String>> = use_signal(Vec::new);
    let mut ollama_embed_models: Signal<Vec<String>> = use_signal(Vec::new);

    // 嵌入模型下载状态
    let model_ready: Signal<bool> = use_signal(|| crate::services::model_downloader::is_model_ready());
    let model_downloading: Signal<bool> = use_signal(|| false);
    let model_progress: Signal<String> = use_signal(String::new);
    let model_progress_percent: Signal<u32> = use_signal(|| 0u32);
    let model_error: Signal<String> = use_signal(String::new);

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
        let mut conv_for_provider = conv_store.clone();
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
            // 仅在切换回内置模式时重建（切换到 ollama 时 embed_model 尚为空，
            // 应等用户选完模型后由 on_select_embed_model 触发 reinit_embedder）
            if !is_ollama {
                let mut ui = ui_store.clone();
                let mut conv = conv_for_provider.clone();
                spawn(async move {
                    let rebuilt = conv.reinit_embedder().await;
                    if rebuilt {
                        let msg = t!("toast.turns-rebuilt").to_string();
                        ui.show_toast(crate::stores::ui::ToastKind::Info, msg, 5000);
                    }
                });
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
                        Tooltip {
                            text: t!("settings.embed-provider-hint").to_string(),
                            span {
                                class: "embed-provider-hint-icon",
                                Icon { data: tabler::InfoCircle, size: "16" }
                            }
                        }
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
                        Tooltip {
                            text: t!("settings.embed-model-hint").to_string(),
                            span {
                                class: "embed-provider-hint-icon",
                                Icon { data: tabler::InfoCircle, size: "16" }
                            }
                        }
                    }
                    {
                        let is_builtin = current_embed_provider != "ollama";
                        let embed_model_opts = if is_builtin {
                            // 内置模式：单选项，显示实际模型名
                            vec![("default".to_string(), "Qwen3-Embedding-0.6B".to_string())]
                        } else {
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
                        };
                        let current_embed_model = if is_builtin {
                            "default".to_string()
                        } else {
                            app_store.config.read().as_ref()
                                .map(|c| c.preferences.ollama.embed_model.clone())
                                .unwrap_or_default()
                        };
                        rsx! {
                            {
                                let mut conv_for_model = conv_store.clone();
                                rsx! {
                            CustomSelect {
                                options: embed_model_opts,
                                value: current_embed_model,
                                disabled: is_builtin,
                                on_select: move |v: String| {
                                    app_store.update_config(|config| {
                                        config.preferences.ollama.embed_model = v;
                                    });
                                    // ollama 嵌入模型变更时也触发重建
                                    let mut ui = ui_store.clone();
                                    let mut conv = conv_for_model.clone();
                                    spawn(async move {
                                        let rebuilt = conv.reinit_embedder().await;
                                        if rebuilt {
                                            let msg = t!("toast.turns-rebuilt").to_string();
                                            ui.show_toast(crate::stores::ui::ToastKind::Info, msg, 5000);
                                        }
                                    });
                                },
                            }
                                }
                            }
                        }
                    }
                }
            }
            // 嵌入模型下载状态（仅在使用内置 Qwen3-Embedding 时显示）
            if current_embed_provider != "ollama" {
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        {t!("settings.embed-model-status").to_string()}
                        Tooltip {
                            text: t!("settings.embed-status-hint").to_string(),
                            span {
                                class: "embed-provider-hint-icon",
                                Icon { data: tabler::InfoCircle, size: "16" }
                            }
                        }
                    }
                    div {
                        class: "{css::embed_model_status}",
                        if model_ready() {
                            // 已就绪：状态 + 重建按钮
                            div {
                                class: "{css::embed_status_row}",
                                span {
                                    class: "{css::embed_status_text_success}",
                                    {t!("settings.embed-model-ready").to_string()}
                                }
                                span {
                                    class: "{css::embed_status_icon}",
                                    Icon { data: tabler::Check, size: "14" }
                                }
                                button {
                                    class: "{css::embed_rebuild_btn}",
                                    onclick: move |_| {
                                        ui_store.show_rebuild_modal.set(true);
                                    },
                                    {t!("settings.rebuild_vectors").to_string()}
                                }
                            }
                        } else if model_downloading() {
                            // 下载中：进度条（高度同按钮，内含百分比文字）
                            div {
                                class: "{css::embed_progress_bar}",
                                div {
                                    class: "{css::embed_progress_fill}",
                                    style: "width:{model_progress_percent()}%;",
                                }
                                div {
                                    class: "{css::embed_progress_label}",
                                    span {
                                        class: "{css::embed_progress_icon}",
                                        Icon { data: tabler::Loader, size: "14" }
                                    }
                                    {model_progress()}
                                }
                            }
                        } else {
                            // 未下载 / 失败：按钮行
                            div {
                                class: "{css::embed_action_row}",
                                button {
                                    class: "{css::embed_download_btn}",
                                    onclick: {
                                        let mut model_downloading = model_downloading;
                                        let mut model_progress = model_progress;
                                        let mut model_progress_percent = model_progress_percent;
                                        let model_ready = model_ready;
                                        let mut model_error = model_error;
                                        let conv_for_download = conv_store.clone();
                                        move |_| {
                                            model_downloading.set(true);
                                            model_error.set(String::new());
                                            model_progress_percent.set(0);
                                            model_progress.set(t!("settings.embed-model-downloading", percent = 0).to_string());
                                            let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::services::model_downloader::DownloadProgress>(32);
                                            let mut mp = model_progress;
                                            let mut mpp = model_progress_percent;
                                            let mut mr = model_ready;
                                            let mut md = model_downloading;
                                            let mut me = model_error;
                                            let mut conv_for_dl = conv_for_download.clone();
                                            spawn(async move {
                                                while let Some(p) = rx.recv().await {
                                                    match p {
                                                        crate::services::model_downloader::DownloadProgress::Downloading(downloaded, total) => {
                                                            let pct = if total > 0 {
                                                                (downloaded as f64 / total as f64 * 100.0) as u32
                                                            } else {
                                                                0
                                                            };
                                                            mpp.set(pct);
                                                            mp.set(t!("settings.embed-model-downloading", percent = pct).to_string());
                                                        }
                                                        crate::services::model_downloader::DownloadProgress::Completed => {
                                                            mr.set(true);
                                                            md.set(false);
                                                            mpp.set(100);
                                                            // 下载完成后立即加载嵌入模型
                                                            let mut conv = conv_for_dl.clone();
                                                            spawn(async move {
                                                                conv.reinit_embedder().await;
                                                            });
                                                            break;
                                                        }
                                                        crate::services::model_downloader::DownloadProgress::Failed(msg) => {
                                                            me.set(msg);
                                                            md.set(false);
                                                            break;
                                                        }
                                                    }
                                                }
                                            });
                                            let tx_err = tx.clone();
                                            spawn(async move {
                                                let on_progress = std::sync::Arc::new(move |p: crate::services::model_downloader::DownloadProgress| {
                                                    let _ = tx.try_send(p);
                                                });
                                                match crate::services::model_downloader::download_model(on_progress).await {
                                                    Ok(_) => {}
                                                    Err(e) => {
                                                        let _ = tx_err.try_send(crate::services::model_downloader::DownloadProgress::Failed(e.to_string()));
                                                    }
                                                }
                                            });
                                        }
                                    },
                                    {t!("settings.embed-model-download").to_string()}
                                }
                                span {
                                    class: "{css::embed_model_size}",
                                    {t!("settings.embed-model-size").to_string()}
                                }
                            }
                            // 错误提示（失败时在按钮下方显示）
                            if !model_error().is_empty() {
                                div {
                                    class: "{css::embed_error_card}",
                                    span {
                                        class: "{css::embed_error_icon}",
                                        Icon { data: tabler::AlertCircle, size: "14" }
                                    }
                                    span {
                                        class: "{css::embed_error_msg}",
                                        {model_error()}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Ollama 模式下的重建向量按钮
            if current_embed_provider == "ollama" {
                div {
                    class: "{css::form_row}",
                    label {
                        class: "{css::form_label}",
                        {t!("settings.embed-model-status").to_string()}
                        Tooltip {
                            text: t!("settings.embed-status-hint").to_string(),
                            span {
                                class: "embed-provider-hint-icon",
                                Icon { data: tabler::InfoCircle, size: "16" }
                            }
                        }
                    }
                    div {
                        class: "{css::embed_model_status}",
                        div {
                            class: "{css::embed_status_row}",
                            span {
                                class: "{css::embed_status_text_success}",
                                {t!("settings.embed-model-ready").to_string()}
                            }
                            span {
                                class: "{css::embed_status_icon}",
                                Icon { data: tabler::Check, size: "14" }
                            }
                            button {
                                class: "{css::embed_rebuild_btn}",
                                onclick: move |_| {
                                    ui_store.show_rebuild_modal.set(true);
                                },
                                {t!("settings.rebuild_vectors").to_string()}
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
                        Tooltip {
                            text: t!("settings.chat-provider-hint").to_string(),
                            span {
                                class: "embed-provider-hint-icon",
                                Icon { data: tabler::InfoCircle, size: "16" }
                            }
                        }
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
                        Tooltip {
                            text: t!("settings.chat-model-hint").to_string(),
                            span {
                                class: "embed-provider-hint-icon",
                                Icon { data: tabler::InfoCircle, size: "16" }
                            }
                        }
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
