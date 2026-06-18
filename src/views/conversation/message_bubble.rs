use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::MessageRole;
use crate::MessageStatus;
use crate::icons::{Icon, tabler};
use crate::components::markdown::Markdown;

/// 解析推理内容文本。
///
/// 优先使用传入的流式推理内容，否则使用消息自身的推理内容，最后回退为空字符串。
#[inline]
pub fn resolve_reasoning_text(streaming_reasoning: &Option<String>, message_reasoning: &Option<String>) -> String {
    streaming_reasoning.as_ref()
        .or(message_reasoning.as_ref())
        .cloned()
        .unwrap_or_default()
}

/// 渲染推理过程折叠区域。
fn render_reasoning_section(
    reasoning_text: &str,
    mut reasoning_expanded: Signal<bool>,
    reasoning_class: dioxus_style::CssClass,
    reasoning_header_class: dioxus_style::CssClass,
    reasoning_body_class: dioxus_style::CssClass,
) -> Element {
    rsx! {
        div {
            class: "{reasoning_class}",
            div {
                class: "{reasoning_header_class}",
                onclick: move |_| reasoning_expanded.set(!reasoning_expanded()),
                span { {t!("chat.reasoning").to_string()} },
                Icon {
                    data: if reasoning_expanded() { tabler::ChevronDown } else { tabler::ChevronRight },
                    size: "14",
                }
            }
            if reasoning_expanded() {
                div {
                    class: "{reasoning_body_class}",
                    Markdown {
                        content: reasoning_text.to_string()
                    }
                }
            }
        }
    }
}

/// 渲染消息操作区域（截断提示等）。
fn render_message_actions(
    status: MessageStatus,
    truncated_hint_class: dioxus_style::CssClass,
) -> Element {
    if status == MessageStatus::Truncated {
        rsx! {
            div {
                class: "{truncated_hint_class}",
                Icon { data: tabler::PlayerStop, size: "12" }
                {t!("chat.status-truncated").to_string()}
            }
        }
    } else {
        rsx! { {} }
    }
}

/// 渲染头像图标。
fn render_avatar_icon(is_user: bool) -> Element {
    if is_user {
        rsx! { Icon { data: tabler::User, size: "24" } }
    } else {
        rsx! { Icon { data: tabler::Robot, size: "24" } }
    }
}

/// 获取消息行的 CSS 类名组合。
fn row_classes(is_user: bool, base: dioxus_style::CssClass, user: dioxus_style::CssClass) -> dioxus_style::CssClass {
    if is_user { base + user } else { base }
}

/// 获取头像的 CSS 类名组合。
fn avatar_classes(is_user: bool, base: dioxus_style::CssClass, user: dioxus_style::CssClass, assistant: dioxus_style::CssClass) -> dioxus_style::CssClass {
    if is_user { base + user } else { base + assistant }
}

/// 获取消息体的 CSS 类名组合。
fn body_classes(is_user: bool, base: dioxus_style::CssClass, user: dioxus_style::CssClass, assistant: dioxus_style::CssClass) -> dioxus_style::CssClass {
    if is_user { base + user } else { base + assistant }
}

/// 获取用户气泡 CSS 类。
fn user_bubble_class(base: dioxus_style::CssClass, user: dioxus_style::CssClass) -> dioxus_style::CssClass {
    base + user
}

/// 获取助手气泡 CSS 类（根据状态选择错误或正常样式）。
fn assistant_bubble_class(status: MessageStatus, base: dioxus_style::CssClass, error: dioxus_style::CssClass, assistant: dioxus_style::CssClass) -> dioxus_style::CssClass {
    if status == MessageStatus::Failed { base + error } else { base + assistant }
}

#[with_css(css, "styles/components/conversation.scss")]
#[component]
pub fn MessageBubble(
    message: crate::Message,
    /// 预解析的推理内容文本（已合并流式和持久化来源）。
    reasoning_text: String,
) -> Element {
    let is_user = message.role == MessageRole::User;
    let reasoning_expanded = use_signal(|| false);
    let has_reasoning = !reasoning_text.is_empty();

    let row_class = row_classes(is_user, css::conv_msg_row, css::conv_msg_row_user);
    let avatar_class = avatar_classes(is_user, css::conv_msg_avatar, css::conv_msg_avatar_user, css::conv_msg_avatar_assistant);
    let body_class = body_classes(is_user, css::conv_msg_body, css::conv_msg_body_user, css::conv_msg_body_assistant);
    let bubble_class = if is_user {
        user_bubble_class(css::conv_msg_bubble, css::conv_msg_bubble_user)
    } else {
        assistant_bubble_class(message.status, css::conv_msg_bubble, css::conv_msg_bubble_error, css::conv_msg_bubble_assistant)
    };

    rsx! {
        div {
            class: "{css::conv_message_canvas}",
            div {
                class: "{row_class}",
                div {
                    class: "{avatar_class}",
                    {render_avatar_icon(is_user)}
                }
                div {
                    class: "{body_class}",
                    div {
                        class: "{bubble_class} msg-content",
                        {render_bubble_content(
                            has_reasoning,
                            &reasoning_text,
                            reasoning_expanded,
                            &message.content,
                            css::conv_msg_reasoning,
                            css::conv_msg_reasoning_header,
                            css::conv_msg_reasoning_body,
                        )}
                    }
                    {render_message_actions(
                        message.status,
                        css::conv_msg_truncated_hint,
                    )}
                }
            }
        }
    }
}

/// 渲染气泡内容区域（推理 + 正文）。
fn render_bubble_content(
    has_reasoning: bool,
    reasoning_text: &str,
    reasoning_expanded: Signal<bool>,
    content: &str,
    reasoning_class: dioxus_style::CssClass,
    reasoning_header_class: dioxus_style::CssClass,
    reasoning_body_class: dioxus_style::CssClass,
) -> Element {
    rsx! {
        if has_reasoning {
            {render_reasoning_section(
                reasoning_text,
                reasoning_expanded,
                reasoning_class,
                reasoning_header_class,
                reasoning_body_class,
            )}
        }
        Markdown { content: content.to_string() }
    }
}
