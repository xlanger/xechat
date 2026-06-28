//! 设置页面 - Ollama 提供商配置区段。
//!
//! 采用 Collapse 可展开样式，包含服务地址配置和对话模型扩展。
//! 对话模型用于生成回复（非嵌入模型），支持添加/删除模型及参数配置。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_app;
use crate::components::input::{Input, InputType};
use crate::components::collapse::Collapse;
use crate::components::tooltip::Tooltip;
use crate::icons::{Icon, tabler};
use crate::models::ModelConfig;

/// 探测状态枚举。
#[derive(Clone, Copy, PartialEq)]
enum ProbeStatus {
    None,
    Loading,
    Ok,
    Fail,
}

fn get_ollama_host(config: &Option<crate::models::config::XEChatConfig>) -> String {
    config.as_ref().map(|c| c.preferences.ollama.host.clone()).unwrap_or_default()
}

/// Ollama 单个对话模型的参数编辑组件。
#[with_css(css, "styles/components/settings.scss")]
#[component]
fn OllamaModelParams(model_name: String) -> Element {
    let mut app_store = use_app();

    let max_tokens_text = t!("settings.max-tokens").to_string();
    let temperature_text = t!("settings.temperature").to_string();
    let top_p_text = t!("settings.top-p").to_string();
    let frequency_penalty_text = t!("settings.frequency-penalty").to_string();
    let presence_penalty_text = t!("settings.presence-penalty").to_string();
    let context_window_text = t!("settings.context-window").to_string();
    let stop_sequences_text = t!("settings.stop-sequences").to_string();
    let stop_sequences_placeholder = t!("settings.stop-sequences-placeholder").to_string();
    let delete_model_text = t!("settings.delete-model").to_string();

    let model_config = app_store
        .config
        .read()
        .as_ref()
        .and_then(|c| c.model_providers.get("ollama"))
        .and_then(|p| p.models.get(&model_name))
        .cloned();

    let Some(mc) = model_config else {
        return rsx! {};
    };

    let default_max_tokens = mc.max_tokens;
    let default_temperature = mc.temperature;
    let default_top_p = mc.top_p;
    let default_frequency_penalty = mc.frequency_penalty;
    let default_presence_penalty = mc.presence_penalty;
    let default_context_window = mc.context_window;
    let default_stop_sequences = mc.stop_sequences.join(",");

    rsx! {
        div {
            class: "{css::model_subsection}",
            div {
                class: "{css::model_name_row}",
                h4 {
                    class: "{css::model_name}",
                    "{model_name}"
                }
                button {
                    class: "{css::model_delete_btn}",
                    title: "{delete_model_text}",
                    onclick: {
                        let mn = model_name.clone();
                        move |_| {
                            let mn = mn.clone();
                            app_store.update_config(|config| {
                                if let Some(provider) = config.model_providers.get_mut("ollama") {
                                    provider.models.remove(&mn);
                                }
                                if config.model == mn && config.model_provider == "ollama"
                                    && let Some(provider) = config.model_providers.get("ollama")
                                        && let Some(first_model) = provider.models.keys().next() {
                                            config.model = first_model.clone();
                                        }
                            });
                        }
                    },
                    "×"
                }
            }

            // Row 1: max_tokens, temperature, top_p, frequency_penalty, presence_penalty
            div {
                class: "{css::model_params_row}",

                div {
                    class: "{css::form_row_compact}",
                    label { class: "{css::form_label_compact}", "{max_tokens_text}" }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get("ollama"))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.max_tokens.to_string())
                            .unwrap_or_else(|| default_max_tokens.to_string());
                        let mn = model_name.clone();
                        rsx! {
                            Input {
                                value: value,
                                placeholder: default_max_tokens.to_string(),
                                input_type: InputType::Number,
                                min: Some(1.0),
                                on_input: move |v: String| {
                                    if v.is_empty() { return; }
                                    if let Ok(val) = v.parse::<u32>() {
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut("ollama")
                                                && let Some(model) = provider.models.get_mut(&mn) {
                                                    model.max_tokens = val;
                                                }
                                        });
                                    }
                                },
                            }
                        }
                    }
                }

                div {
                    class: "{css::form_row_compact}",
                    label { class: "{css::form_label_compact}", "{temperature_text}" }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get("ollama"))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.temperature.to_string())
                            .unwrap_or_else(|| default_temperature.to_string());
                        let mn = model_name.clone();
                        rsx! {
                            Input {
                                value: value,
                                placeholder: default_temperature.to_string(),
                                input_type: InputType::Number,
                                min: Some(0.0),
                                max: Some(2.0),
                                on_input: move |v: String| {
                                    if v.is_empty() { return; }
                                    if let Ok(val) = v.parse::<f32>() {
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut("ollama")
                                                && let Some(model) = provider.models.get_mut(&mn) {
                                                    model.temperature = val;
                                                }
                                        });
                                    }
                                },
                            }
                        }
                    }
                }

                div {
                    class: "{css::form_row_compact}",
                    label { class: "{css::form_label_compact}", "{top_p_text}" }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get("ollama"))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.top_p.to_string())
                            .unwrap_or_else(|| default_top_p.to_string());
                        let mn = model_name.clone();
                        rsx! {
                            Input {
                                value: value,
                                placeholder: default_top_p.to_string(),
                                input_type: InputType::Number,
                                min: Some(0.0),
                                max: Some(1.0),
                                on_input: move |v: String| {
                                    if v.is_empty() { return; }
                                    if let Ok(val) = v.parse::<f32>() {
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut("ollama")
                                                && let Some(model) = provider.models.get_mut(&mn) {
                                                    model.top_p = val;
                                                }
                                        });
                                    }
                                },
                            }
                        }
                    }
                }

                div {
                    class: "{css::form_row_compact}",
                    label { class: "{css::form_label_compact}", "{frequency_penalty_text}" }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get("ollama"))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.frequency_penalty.to_string())
                            .unwrap_or_else(|| default_frequency_penalty.to_string());
                        let mn = model_name.clone();
                        rsx! {
                            Input {
                                value: value,
                                placeholder: default_frequency_penalty.to_string(),
                                input_type: InputType::Number,
                                min: Some(0.0),
                                max: Some(2.0),
                                on_input: move |v: String| {
                                    if v.is_empty() { return; }
                                    if let Ok(val) = v.parse::<f32>() {
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut("ollama")
                                                && let Some(model) = provider.models.get_mut(&mn) {
                                                    model.frequency_penalty = val;
                                                }
                                        });
                                    }
                                },
                            }
                        }
                    }
                }

                div {
                    class: "{css::form_row_compact}",
                    label { class: "{css::form_label_compact}", "{presence_penalty_text}" }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get("ollama"))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.presence_penalty.to_string())
                            .unwrap_or_else(|| default_presence_penalty.to_string());
                        let mn = model_name.clone();
                        rsx! {
                            Input {
                                value: value,
                                placeholder: default_presence_penalty.to_string(),
                                input_type: InputType::Number,
                                min: Some(0.0),
                                max: Some(2.0),
                                on_input: move |v: String| {
                                    if v.is_empty() { return; }
                                    if let Ok(val) = v.parse::<f32>() {
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut("ollama")
                                                && let Some(model) = provider.models.get_mut(&mn) {
                                                    model.presence_penalty = val;
                                                }
                                        });
                                    }
                                },
                            }
                        }
                    }
                }
            }

            // Row 2: context_window, stop_sequences
            div {
                class: "{css::model_params_row}",

                div {
                    class: "{css::form_row_compact}",
                    label { class: "{css::form_label_compact}", "{context_window_text}" }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get("ollama"))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.context_window.to_string())
                            .unwrap_or_else(|| default_context_window.to_string());
                        let mn = model_name.clone();
                        rsx! {
                            Input {
                                value: value,
                                placeholder: default_context_window.to_string(),
                                input_type: InputType::Number,
                                min: Some(1.0),
                                on_input: move |v: String| {
                                    if v.is_empty() { return; }
                                    if let Ok(val) = v.parse::<u32>() {
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut("ollama")
                                                && let Some(model) = provider.models.get_mut(&mn) {
                                                    model.context_window = val;
                                                }
                                        });
                                    }
                                },
                            }
                        }
                    }
                }

                div {
                    class: "{css::form_row_compact}",
                    label { class: "{css::form_label_compact}", "{stop_sequences_text}" }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get("ollama"))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.stop_sequences.join(","))
                            .unwrap_or_else(|| default_stop_sequences.clone());
                        let mn = model_name.clone();
                        rsx! {
                            Input {
                                value: value,
                                placeholder: stop_sequences_placeholder.clone(),
                                input_type: InputType::Text,
                                on_input: move |v: String| {
                                    let seqs: Vec<String> = if v.is_empty() {
                                        vec![]
                                    } else {
                                        v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                                    };
                                    let mn = mn.clone();
                                    app_store.update_config(|config| {
                                        if let Some(provider) = config.model_providers.get_mut("ollama")
                                            && let Some(model) = provider.models.get_mut(&mn) {
                                                model.stop_sequences = seqs;
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
}

#[with_css(css, "styles/components/settings.scss")]
#[component]
pub fn OllamaSection() -> Element {
    let mut app_store = use_app();

    let section_text = t!("settings.ollama").to_string();
    let host_text = t!("settings.ollama-host").to_string();
    let host_placeholder = t!("settings.ollama-host-placeholder").to_string();
    let chat_models_text = t!("settings.ollama-chat-models").to_string();
    let chat_models_hint = t!("settings.ollama-chat-models-hint").to_string();
    let add_model_text = t!("settings.add-model").to_string();
    let add_model_placeholder = t!("settings.add-model-placeholder").to_string();

    // 探测状态信号
    let mut host_status: Signal<ProbeStatus> = use_signal(|| ProbeStatus::None);

    // 控制内联添加模型输入框的显示
    let mut show_add_model: Signal<bool> = use_signal(|| false);
    // 存储待添加的模型名称
    let mut new_model_name: Signal<String> = use_signal(String::new);

    // 读取当前配置值
    let current_host = get_ollama_host(&app_store.config.read());

    // 读取 ollama provider 的模型列表
    let ollama_models: Vec<String> = app_store
        .config
        .read()
        .as_ref()
        .and_then(|c| c.model_providers.get("ollama"))
        .map(|p| p.models.keys().cloned().collect())
        .unwrap_or_default();

    // 探测 Ollama 服务地址
    {
        let host = current_host.clone();
        use_effect(move || {
            let host = host.clone();
            if host.is_empty() {
                host_status.set(ProbeStatus::None);
                return;
            }
            host_status.set(ProbeStatus::Loading);
            spawn(async move {
                let ok = crate::services::ollama::probe::probe_host(&host).await;
                host_status.set(if ok { ProbeStatus::Ok } else { ProbeStatus::Fail });
            });
        });
    }

    let host_icon = match *host_status.read() {
        ProbeStatus::Ok => Some(rsx! { Icon { data: tabler::Check, size: "16", style: "color: var(--color-success, #22c55e)" } }),
        ProbeStatus::Fail => Some(rsx! { Icon { data: tabler::X, size: "16", style: "color: var(--color-error, #ef4444)" } }),
        _ => None,
    };

    rsx! {
        section {
            class: "{css::settings_section}",
            Collapse {
                title: section_text,
                default_open: false,
                div {
                    class: "{css::provider_content}",

                    // 服务地址
                    div {
                        class: "{css::form_row}",
                        label {
                            class: "{css::form_label}",
                            "{host_text}"
                            Tooltip {
                                text: t!("settings.ollama-host-hint").to_string(),
                                span {
                                    class: "embed-provider-hint-icon",
                                    Icon { data: tabler::InfoCircle, size: "16" }
                                }
                            }
                        }
                        Input {
                            value: current_host,
                            placeholder: host_placeholder,
                            right_icon: host_icon,
                            on_input: move |v: String| {
                                app_store.update_config(|config| {
                                    config.preferences.ollama.host = v;
                                });
                            },
                        }
                    }

                    // 对话模型区域标题
                    div {
                        class: "{css::form_row}",
                        label {
                            class: "{css::form_label}",
                            "{chat_models_text}"
                            Tooltip {
                                text: chat_models_hint,
                                span {
                                    class: "embed-provider-hint-icon",
                                    Icon { data: tabler::InfoCircle, size: "16" }
                                }
                            }
                        }
                    }

                    // 已有模型列表
                    for mn in ollama_models {
                        OllamaModelParams {
                            model_name: mn,
                        }
                    }

                    // 添加模型区域
                    div {
                        class: "{css::add_model_row}",
                        if !show_add_model() {
                            button {
                                class: "{css::add_model_btn}",
                                onclick: move |_| {
                                    show_add_model.set(true);
                                },
                                "+ {add_model_text}"
                            }
                        } else {
                            input {
                                r#type: "text",
                                class: "{css::add_model_input}",
                                placeholder: "{add_model_placeholder}",
                                value: "{new_model_name}",
                                autofocus: true,
                                oninput: move |event| {
                                    new_model_name.set(event.value());
                                },
                                onkeydown: move |event| {
                                    if event.key() == Key::Enter {
                                        let name = new_model_name.read().trim().to_string();
                                        if name.is_empty() { return; }
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut("ollama") {
                                                provider.models.entry(name).or_insert_with(|| ModelConfig {
                                                        max_tokens: 4096,
                                                        temperature: 0.7,
                                                        top_p: 0.9,
                                                        frequency_penalty: 0.0,
                                                        presence_penalty: 0.0,
                                                        context_window: 8192,
                                                        stop_sequences: vec![],
                                                    });
                                            }
                                        });
                                        new_model_name.set(String::new());
                                        show_add_model.set(false);
                                    } else if event.key() == Key::Escape {
                                        new_model_name.set(String::new());
                                        show_add_model.set(false);
                                    }
                                },
                            }
                            button {
                                class: "{css::add_model_btn}",
                                onclick: move |_| {
                                    let name = new_model_name.read().trim().to_string();
                                    if name.is_empty() { return; }
                                    app_store.update_config(|config| {
                                        if let Some(provider) = config.model_providers.get_mut("ollama") {
                                            provider.models.entry(name).or_insert_with(|| ModelConfig {
                                                    max_tokens: 4096,
                                                    temperature: 0.7,
                                                    top_p: 0.9,
                                                    frequency_penalty: 0.0,
                                                    presence_penalty: 0.0,
                                                    context_window: 8192,
                                                    stop_sequences: vec![],
                                                });
                                        }
                                    });
                                    new_model_name.set(String::new());
                                    show_add_model.set(false);
                                },
                                "✓"
                            }
                        }
                    }
                }
            }
        }
    }
}
