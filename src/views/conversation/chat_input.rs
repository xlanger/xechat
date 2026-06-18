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

/// 构建所有可用模型的单一选择列表。
///
/// 返回 (选项列表, 当前选中值)。
pub fn build_model_options(config: &crate::XEChatConfig) -> (Vec<(String, String)>, String) {
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

/// 判断是否应发送消息（内容非空且未在流式传输中）。
#[inline]
pub fn should_send(content: &str, is_streaming: bool) -> bool {
    !content.trim().is_empty() && !is_streaming
}

/// 判断按键事件是否应触发发送（Enter 且无 Shift，且内容非空）。
#[inline]
pub fn is_send_key(evt: &KeyboardEvent, has_content: bool) -> bool {
    evt.key() == Key::Enter && !evt.modifiers().shift() && has_content
}

/// 从 "provider/model" 格式的值中解析并更新配置。
///
/// 若格式正确，返回 `Some(())`；格式不匹配返回 `None`。
#[inline]
pub fn apply_model_selection(value: &str, config: &mut crate::XEChatConfig) -> Option<()> {
    let (pk, model) = value.split_once('/')?;
    config.model_provider = pk.to_string();
    config.model = model.to_string();
    Some(())
}

/// 计算发送按钮的 CSS 类名。
fn send_btn_class(
    is_streaming: bool,
    has_content: bool,
    base: dioxus_style::CssClass,
    active: dioxus_style::CssClass,
    disabled: dioxus_style::CssClass,
) -> dioxus_style::CssClass {
    if is_streaming || has_content {
        base + active
    } else {
        base + disabled
    }
}

/// 计算模型触发器的 CSS 类名。
fn model_trigger_class(
    is_open: bool,
    base: dioxus_style::CssClass,
    open: dioxus_style::CssClass,
) -> dioxus_style::CssClass {
    if is_open {
        base + open
    } else {
        base
    }
}

/// 计算模型箭头的 CSS 类名。
fn model_arrow_class(
    is_open: bool,
    base: dioxus_style::CssClass,
    open: dioxus_style::CssClass,
) -> dioxus_style::CssClass {
    if is_open {
        base + open
    } else {
        base
    }
}

/// 处理输入框按键事件，若满足发送条件则调用回调。
fn handle_input_keydown(
    evt: KeyboardEvent,
    has_content: bool,
    mut on_send: impl FnMut(),
) {
    if is_send_key(&evt, has_content) {
        evt.prevent_default();
        on_send();
    }
}

/// 渲染输入框底部工具栏（模型选择器 + 发送按钮）。
fn render_input_actions(
    toolbar_class: dioxus_style::CssClass,
    model_selectors_class: dioxus_style::CssClass,
    model_selector_wrapper_class: dioxus_style::CssClass,
    trigger_class: dioxus_style::CssClass,
    trigger_label_class: dioxus_style::CssClass,
    arrow_class: dioxus_style::CssClass,
    is_model_open: bool,
    dropdown_class: dioxus_style::CssClass,
    option_class: dioxus_style::CssClass,
    option_selected_class: dioxus_style::CssClass,
    add_class: dioxus_style::CssClass,
    selected_label: &str,
    options: &[(String, String)],
    current_value: &str,
    send_btn_class_val: dioxus_style::CssClass,
    is_streaming: bool,
    on_toggle_dropdown: impl FnMut(MouseEvent) + 'static,
    on_select_model: Rc<RefCell<dyn FnMut(String)>>,
    on_goto_settings: impl FnMut(MouseEvent) + 'static,
    on_focusout: impl FnMut(FocusEvent) + 'static,
    on_click_send: impl FnMut(MouseEvent) + 'static,
) -> Element {
    rsx! {
        div {
            class: "{toolbar_class}",
            div {
                class: "{model_selectors_class}",
                div {
                    class: "{model_selector_wrapper_class}",
                    tabindex: "0",
                    onfocusout: on_focusout,
                    div {
                        class: "{trigger_class}",
                        onclick: on_toggle_dropdown,
                        span { class: "{trigger_label_class}", "{selected_label}" }
                        span {
                            class: "{arrow_class}",
                            Icon { data: tabler::ChevronDown, size: "14", stroke: "currentColor" }
                        }
                    }
                    if is_model_open {
                        div {
                            class: "{dropdown_class}",
                            for (k, label) in options {
                                div {
                                    class: if k == current_value {
                                        format!("{} {}", option_class, option_selected_class)
                                    } else {
                                        format!("{}", option_class)
                                    },
                                    onclick: {
                                        let key = k.clone();
                                        let on_select = on_select_model.clone();
                                        move |_| {
                                            (on_select.borrow_mut())(key.clone());
                                        }
                                    },
                                    "{label}"
                                }
                            }
                            div {
                                class: "{add_class}",
                                onclick: on_goto_settings,
                                Icon { data: tabler::Settings, size: "14", stroke: "currentColor" }
                                span { "{t!(\"settings.add-model\")}" }
                            }
                        }
                    }
                }
            }
            div {
                class: "{send_btn_class_val}",
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
    let s_btn_class = send_btn_class(is_streaming, has_content, css::conv_send_btn, css::conv_send_btn_active, css::conv_send_btn_disabled);

    // 构建所有可用模型的单一选择列表："provider_key/model_name" → "模型名" 或 "模型名（OpenAI Compatible）"
    let (model_selector_options, current_selector_value) = {
        let config_guard = app_store.config.read();
        match config_guard.as_ref() {
            Some(config) => build_model_options(config),
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

    let select_model_item: Rc<RefCell<dyn FnMut(String)>> = Rc::new(RefCell::new({
        let mut app_store = app_store.clone();
        let mut model_open = model_open;
        move |v: String| {
            if let Some(config) = app_store.config.write().as_mut() {
                apply_model_selection(&v, config);
            }
            model_open.set(false);
        }
    }));

    let goto_settings = {
        let mut app_store = app_store.clone();
        move |_| {
            model_open.set(false);
            app_store.navigate_to(MainRoute::Settings);
        }
    };

    let is_model_open = *model_open.read();

    let m_trigger_class = model_trigger_class(is_model_open, css::conv_model_trigger, css::conv_model_trigger_open);
    let m_arrow_class = model_arrow_class(is_model_open, css::conv_model_arrow, css::conv_model_arrow_open);

    // 发送消息的公共逻辑，用 Rc<RefCell> 共享于多个闭包
    let send_message: Rc<RefCell<dyn FnMut()>> = Rc::new(RefCell::new({
        let mut conv_store = conv_store.clone();
        let mut app_store = app_store.clone();
        let mut input_value = input_value;
        move || {
            let content = input_value.read().trim().to_string();
            if !should_send(&content, conv_store.streaming()) {
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
            handle_input_keydown(evt, !input_value.read().trim().is_empty(), &mut *send_message.borrow_mut());
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
                {render_input_actions(
                    css::conv_input_toolbar,
                    css::conv_input_model_selectors,
                    css::conv_model_selector_wrapper,
                    m_trigger_class,
                    css::conv_model_trigger_label,
                    m_arrow_class,
                    is_model_open,
                    css::conv_model_dropdown,
                    css::conv_model_option,
                    css::conv_model_option_selected,
                    css::conv_model_add,
                    &selected_model_label,
                    &model_selector_options,
                    &current_selector_value,
                    s_btn_class,
                    is_streaming,
                    toggle_model_dropdown,
                    select_model_item,
                    goto_settings,
                    move |_| model_open.set(false),
                    on_click_send,
                )}
            }
        }
    }
}
