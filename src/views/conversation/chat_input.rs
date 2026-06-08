//! 对话输入框组件。
//!
//! 提供多行文本输入、内联模型选择器和提交/停止按钮。
//! 布局：textarea 占满宽度，底部工具栏左对齐模型选择器、右对齐提交按钮。
//! 支持在 Welcome 页使用：发送时若无当前对话则自动创建并导航。

use std::cell::RefCell;
use std::rc::Rc;
use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_conversation, use_app};
use crate::state::MainRoute;
use crate::icons::{Icon, tabler};

#[with_css(css, "styles/components/conversation.scss")]
#[component]
pub fn ChatInput() -> Element {
    let conv_store = use_conversation();
    let app_store = use_app();
    let _lang = *app_store.language.read();

    let mut input_value = use_signal(String::new);
    let placeholder = t!("chat.input-placeholder");

    let is_streaming = conv_store.streaming();

    let has_content = !input_value.read().trim().is_empty();
    let send_btn_class = if is_streaming {
        css::conv_send_btn + css::conv_send_btn_active
    } else if has_content {
        css::conv_send_btn + css::conv_send_btn_active
    } else {
        css::conv_send_btn + css::conv_send_btn_disabled
    };

    // 构建所有可用模型的单一选择列表："provider_key/model_name" → "模型名" 或 "模型名（OpenAI Compatible）"
    let (model_selector_options, current_selector_value) = {
        let config_guard = app_store.config.read();
        match config_guard.as_ref() {
            Some(config) => {
                let mut opts: Vec<(String, String)> = Vec::new();
                for (pk, provider) in config.model_providers.iter() {
                    let is_compatible = !matches!(pk.as_str(), "deepseek" | "openai" | "ollama");
                    for model_name in provider.models.keys() {
                        let display = if is_compatible {
                            format!("{}（{}）", model_name, t!("settings.openai-compatible"))
                        } else {
                            model_name.clone()
                        };
                        let value = format!("{}/{}", pk, model_name);
                        opts.push((value, display));
                    }
                }
                let cur_val = format!("{}/{}", config.model_provider, config.model);
                (opts, cur_val)
            }
            None => (Vec::new(), String::new()),
        }
    };

    // ---- 模型选择器状态 ----
    let mut model_open = use_signal(|| false);

    let selected_model_label = model_selector_options.iter()
        .find(|(k, _)| k == &current_selector_value)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| current_selector_value.clone());

    let toggle_model_dropdown = move |e: MouseEvent| {
        e.stop_propagation();
        let is = *model_open.read();
        model_open.set(!is);
    };

    let mut select_model_item = {
        let mut app_store = app_store.clone();
        let mut model_open = model_open;
        move |v: String| {
            if let Some((pk, model)) = v.split_once('/') {
                app_store.update_config(|config| {
                    config.model_provider = pk.to_string();
                    config.model = model.to_string();
                });
            }
            model_open.set(false);
        }
    };

    let goto_settings = {
        let mut app_store = app_store.clone();
        move |_| {
            model_open.set(false);
            app_store.navigate_to(MainRoute::Settings);
        }
    };

    let is_model_open = *model_open.read();

    let model_trigger_class = if is_model_open {
        format!("{} {}", css::conv_model_trigger, css::conv_model_trigger_open)
    } else {
        format!("{}", css::conv_model_trigger)
    };

    let model_arrow_class = if is_model_open {
        format!("{} {}", css::conv_model_arrow, css::conv_model_arrow_open)
    } else {
        format!("{}", css::conv_model_arrow)
    };

    // 发送消息的公共逻辑，用 Rc<RefCell> 共享于多个闭包
    let send_message: Rc<RefCell<dyn FnMut()>> = Rc::new(RefCell::new({
        let mut conv_store = conv_store.clone();
        let mut app_store = app_store.clone();
        let mut input_value = input_value;
        move || {
            let content = input_value.read().trim().to_string();
            if content.is_empty() || conv_store.streaming() {
                return;
            }
            // 若无当前对话，先创建
            if conv_store.current_conversation_id.read().is_none() {
                let title = t!("conversation.default-title").into_owned();
                let conv_id = conv_store.create_temporary_conversation(title);
                app_store.navigate_to(MainRoute::Conversation(conv_id));
            }
            input_value.set(String::new());
            let config = match app_store.config.read().as_ref() {
                Some(c) => c.clone(),
                None => return,
            };
            // 通过 pending_send 信号将发送请求传递给 Layout 处理，
            // 避免 spawn 被 Welcome 页 unmount 取消
            conv_store.pending_send.set(Some((content, config)));
        }
    }));

    let on_keydown_send = {
        let send_message = send_message.clone();
        move |evt: KeyboardEvent| {
            if evt.key() == Key::Enter && !evt.modifiers().shift() && !input_value.read().trim().is_empty() {
                evt.prevent_default();
                (send_message.borrow_mut())();
            }
        }
    };

    let on_click_send = {
        let send_message = send_message.clone();
        let mut conv_store = conv_store.clone();
        move |_| {
            if conv_store.streaming() {
                conv_store.stop_streaming();
                return;
            }
            (send_message.borrow_mut())();
        }
    };

    rsx! {
        div {
            class: "{css::conv_input_container}",
            div {
                class: "{css::conv_input_wrapper}",
                textarea {
                    class: "{css::conv_input_textarea}",
                    placeholder: "{placeholder}",
                    value: "{input_value}",
                    id: "xechat-chat-input",
                    r#onmounted: move |_| {
                        let _ = dioxus::desktop::window().webview.evaluate_script(
                            "document.getElementById('xechat-chat-input')?.focus()"
                        );
                    },
                    oninput: move |e: FormEvent| {
                        input_value.set(e.value());
                        // 自动调整高度：先重置再根据 scrollHeight 设置
                        let _ = dioxus::desktop::window().webview.evaluate_script(
                            "var ta=document.activeElement;if(ta&&ta.tagName==='TEXTAREA'){ta.style.height='auto';ta.style.height=Math.min(ta.scrollHeight,168)+'px';}"
                        );
                    },
                    onkeydown: on_keydown_send,
                }
                div {
                    class: "{css::conv_input_toolbar}",
                    div {
                        class: "{css::conv_input_model_selectors}",
                        // 内联模型选择器
                        div {
                            class: "{css::conv_model_selector_wrapper}",
                            tabindex: "0",
                            onfocusout: move |_| model_open.set(false),
                            div {
                                class: "{model_trigger_class}",
                                onclick: toggle_model_dropdown,
                                span { class: "{css::conv_model_trigger_label}", "{selected_model_label}" }
                                span {
                                    class: "{model_arrow_class}",
                                    Icon { data: tabler::ChevronDown, size: "14", stroke: "currentColor" }
                                }
                            }
                            if is_model_open {
                                div {
                                    class: "{css::conv_model_dropdown}",
                                    for (k, label) in &model_selector_options {
                                        div {
                                            class: if k == &current_selector_value {
                                                format!("{} {}", css::conv_model_option, css::conv_model_option_selected)
                                            } else {
                                                format!("{}", css::conv_model_option)
                                            },
                                            onclick: {
                                                let key = k.clone();
                                                move |_| select_model_item(key.clone())
                                            },
                                            "{label}"
                                        }
                                    }
                                    div {
                                        class: "{css::conv_model_add}",
                                        onclick: goto_settings,
                                        Icon { data: tabler::Settings, size: "14", stroke: "currentColor" }
                                        span { "{t!(\"settings.add-model\")}" }
                                    }
                                }
                            }
                        }
                    }
                    div {
                        class: "{send_btn_class}",
                        onclick: on_click_send,
                        if is_streaming {
                            Icon { data: tabler::PlayerStop, size: "16", stroke: "white" }
                        } else {
                            Icon { data: tabler::Send, size: "16", stroke: "white" }
                        }
                    }
                }
            }
        }
    }
}
