//! 设置页面 - 模型提供商配置区段。
//!
//! 通用的提供商配置组件，通过 `provider_key` 参数适配任意提供商。
//! 自动遍历 `provider.models` HashMap 渲染每个模型的参数编辑界面。
//! 支持添加新模型和删除已有模型。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_app;
use crate::components::input::{Input, InputType};
use crate::components::collapse::Collapse;
use crate::models::ModelConfig;

#[with_css(css, "styles/components/settings.scss")]
#[component]
pub fn ModelParamsSection(provider_key: String, model_name: String) -> Element {
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
        .and_then(|c| c.model_providers.get(&provider_key))
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

    let display_name = model_name.clone();

    rsx! {
        div {
            class: "{css::model_subsection}",
            div {
                class: "{css::model_name_row}",
                h4 {
                    class: "{css::model_name}",
                    "{display_name}"
                }
                button {
                    class: "{css::model_delete_btn}",
                    title: "{delete_model_text}",
                    onclick: {
                        let pk = provider_key.clone();
                        let mn = model_name.clone();
                        move |_| {
                            let pk = pk.clone();
                            let mn = mn.clone();
                            app_store.update_config(|config| {
                                if let Some(provider) = config.model_providers.get_mut(&pk) {
                                    provider.models.remove(&mn);
                                }
                                // 如果删除的是当前使用的模型，自动切换到同 Provider 下的第一个模型
                                if config.model == mn && config.model_provider == pk {
                                    if let Some(provider) = config.model_providers.get(&pk) {
                                        if let Some(first_model) = provider.models.keys().next() {
                                            config.model = first_model.clone();
                                        }
                                    }
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
                    label {
                        class: "{css::form_label_compact}",
                        "{max_tokens_text}"
                    }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&provider_key))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.max_tokens.to_string())
                            .unwrap_or_else(|| default_max_tokens.to_string());
                        let pk = provider_key.clone();
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
                                        let pk = pk.clone();
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk)
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
                    label {
                        class: "{css::form_label_compact}",
                        "{temperature_text}"
                    }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&provider_key))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.temperature.to_string())
                            .unwrap_or_else(|| default_temperature.to_string());
                        let pk = provider_key.clone();
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
                                        let pk = pk.clone();
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk)
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
                    label {
                        class: "{css::form_label_compact}",
                        "{top_p_text}"
                    }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&provider_key))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.top_p.to_string())
                            .unwrap_or_else(|| default_top_p.to_string());
                        let pk = provider_key.clone();
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
                                        let pk = pk.clone();
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk)
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
                    label {
                        class: "{css::form_label_compact}",
                        "{frequency_penalty_text}"
                    }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&provider_key))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.frequency_penalty.to_string())
                            .unwrap_or_else(|| default_frequency_penalty.to_string());
                        let pk = provider_key.clone();
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
                                        let pk = pk.clone();
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk)
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
                    label {
                        class: "{css::form_label_compact}",
                        "{presence_penalty_text}"
                    }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&provider_key))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.presence_penalty.to_string())
                            .unwrap_or_else(|| default_presence_penalty.to_string());
                        let pk = provider_key.clone();
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
                                        let pk = pk.clone();
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk)
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
                    label {
                        class: "{css::form_label_compact}",
                        "{context_window_text}"
                    }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&provider_key))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.context_window.to_string())
                            .unwrap_or_else(|| default_context_window.to_string());
                        let pk = provider_key.clone();
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
                                        let pk = pk.clone();
                                        let mn = mn.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk)
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
                    label {
                        class: "{css::form_label_compact}",
                        "{stop_sequences_text}"
                    }
                    {
                        let value = app_store.config.read().as_ref()
                            .and_then(|c| c.model_providers.get(&provider_key))
                            .and_then(|p| p.models.get(&model_name))
                            .map(|m| m.stop_sequences.join(","))
                            .unwrap_or_else(|| default_stop_sequences.clone());
                        let pk = provider_key.clone();
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
                                    let pk = pk.clone();
                                    let mn = mn.clone();
                                    app_store.update_config(|config| {
                                        if let Some(provider) = config.model_providers.get_mut(&pk)
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
pub fn ProviderSection(provider_key: String) -> Element {
    let mut app_store = use_app();

    let api_key_text = t!("settings.api-key").to_string();
    let base_url_text = t!("settings.base-url").to_string();
    let timeout_text = t!("settings.timeout").to_string();
    let api_key_placeholder = t!("settings.api-key-placeholder").to_string();
    let add_model_text = t!("settings.add-model").to_string();
    let add_model_placeholder = t!("settings.add-model-placeholder").to_string();

    // 控制内联添加模型输入框的显示
    let mut show_add_model: Signal<bool> = use_signal(|| false);
    // 存储待添加的模型名称
    let mut new_model_name: Signal<String> = use_signal(String::new);

    let provider = app_store
        .config
        .read()
        .as_ref()
        .and_then(|c| c.model_providers.get(&provider_key))
        .cloned();

    let Some(provider) = provider else {
        return rsx! {};
    };

    let provider_name = provider.name.clone();
    let default_base_url = provider.base_url.clone();

    let model_names: Vec<String> = provider.models.keys().cloned().collect();

    rsx! {
        section {
            class: "{css::settings_section}",
            Collapse {
                title: "{provider_name}",
                default_open: true,
                div {
                    class: "{css::provider_content}",

                    div {
                        class: "{css::form_row_inline}",
                        div {
                            class: "{css::form_row}",
                            label {
                                class: "{css::form_label}",
                                "{base_url_text}"
                            }
                            {
                                let value = app_store.config.read().as_ref()
                                    .and_then(|c| c.model_providers.get(&provider_key))
                                    .map(|p| p.base_url.clone())
                                    .unwrap_or_else(|| default_base_url.clone());
                                let pk = provider_key.clone();
                                let placeholder = default_base_url.clone();
                                rsx! {
                                    Input {
                                        value: value,
                                        placeholder: placeholder.clone(),
                                        input_type: InputType::Text,
                                        on_input: move |v: String| {
                                            let pk = pk.clone();
                                            app_store.update_config(|config| {
                                                if let Some(provider) = config.model_providers.get_mut(&pk) {
                                                    provider.base_url = v;
                                                }
                                            });
                                        },
                                    }
                                }
                            }
                        }

                        div {
                            class: "{css::form_row}",
                            label {
                                class: "{css::form_label}",
                                "{timeout_text}"
                            }
                            {
                                let value = app_store.config.read().as_ref()
                                    .and_then(|c| c.model_providers.get(&provider_key))
                                    .and_then(|p| p.timeout)
                                    .map(|t| t.to_string())
                                    .unwrap_or_else(|| "120".to_string());
                                let pk = provider_key.clone();
                                rsx! {
                                    Input {
                                        value: value,
                                        placeholder: "120".to_string(),
                                        input_type: InputType::Number,
                                        on_input: move |v: String| {
                                            let pk = pk.clone();
                                            app_store.update_config(|config| {
                                                if let Some(provider) = config.model_providers.get_mut(&pk) {
                                                    provider.timeout = v.parse::<u64>().ok();
                                                }
                                            });
                                        },
                                    }
                                }
                            }
                        }
                    }

                    div {
                        class: "{css::form_row}",
                        label {
                            class: "{css::form_label}",
                            "{api_key_text}"
                        }
                        {
                            let value = app_store.config.read().as_ref()
                                .and_then(|c| c.model_providers.get(&provider_key))
                                .map(|p| p.api_key.clone())
                                .unwrap_or_default();
                            let pk = provider_key.clone();
                            rsx! {
                                Input {
                                    value: value,
                                    placeholder: api_key_placeholder.clone(),
                                    input_type: InputType::Password,
                                    on_input: move |v: String| {
                                        let pk = pk.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk) {
                                                provider.api_key = v;
                                            }
                                        });
                                    },
                                }
                            }
                        }
                    }

                    for mn in model_names {
                        ModelParamsSection {
                            provider_key: provider_key.clone(),
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
                                onkeydown: {
                                    let pk = provider_key.clone();
                                    move |event| {
                                        if event.key() == Key::Enter {
                                            let name = new_model_name.read().trim().to_string();
                                            if name.is_empty() { return; }
                                            let pk = pk.clone();
                                            app_store.update_config(|config| {
                                                if let Some(provider) = config.model_providers.get_mut(&pk) {
                                                    if !provider.models.contains_key(&name) {
                                                        provider.models.insert(name, ModelConfig {
                                                            max_tokens: 4096,
                                                            temperature: 0.7,
                                                            top_p: 0.9,
                                                            frequency_penalty: 0.0,
                                                            presence_penalty: 0.0,
                                                            context_window: 8192,
                                                            stop_sequences: vec![],
                                                        });
                                                    }
                                                }
                                            });
                                            new_model_name.set(String::new());
                                            show_add_model.set(false);
                                        } else if event.key() == Key::Escape {
                                            new_model_name.set(String::new());
                                            show_add_model.set(false);
                                        }
                                    }
                                },
                            }
                            button {
                                class: "{css::add_model_btn}",
                                onclick: {
                                    let pk = provider_key.clone();
                                    move |_| {
                                        let name = new_model_name.read().trim().to_string();
                                        if name.is_empty() { return; }
                                        let pk = pk.clone();
                                        app_store.update_config(|config| {
                                            if let Some(provider) = config.model_providers.get_mut(&pk) {
                                                if !provider.models.contains_key(&name) {
                                                    provider.models.insert(name, ModelConfig {
                                                        max_tokens: 4096,
                                                        temperature: 0.7,
                                                        top_p: 0.9,
                                                        frequency_penalty: 0.0,
                                                        presence_penalty: 0.0,
                                                        context_window: 8192,
                                                        stop_sequences: vec![],
                                                    });
                                                }
                                            }
                                        });
                                        new_model_name.set(String::new());
                                        show_add_model.set(false);
                                    }
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
