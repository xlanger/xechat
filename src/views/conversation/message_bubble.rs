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

#[with_css(css, "styles/components/conversation.scss")]
#[component]
pub fn MessageBubble(
    message: crate::Message,
    /// 流式推理过程（仅流式气泡传入，持久化消息使用 message.reasoning_content）
    streaming_reasoning: Option<String>,
) -> Element {
    /// 获取消息行的 CSS 类名组合。
    fn row_classes(is_user: bool) -> dioxus_style::CssClass {
        if is_user {
            css::conv_msg_row + css::conv_msg_row_user
        } else {
            css::conv_msg_row
        }
    }

    /// 获取头像的 CSS 类名组合。
    fn avatar_classes(is_user: bool) -> dioxus_style::CssClass {
        if is_user {
            css::conv_msg_avatar + css::conv_msg_avatar_user
        } else {
            css::conv_msg_avatar + css::conv_msg_avatar_assistant
        }
    }

    /// 获取消息体的 CSS 类名组合。
    fn body_classes(is_user: bool) -> dioxus_style::CssClass {
        if is_user {
            css::conv_msg_body + css::conv_msg_body_user
        } else {
            css::conv_msg_body + css::conv_msg_body_assistant
        }
    }

    /// 获取气泡的 CSS 类名组合。
    fn bubble_classes(is_user: bool, status: MessageStatus) -> dioxus_style::CssClass {
        if is_user {
            css::conv_msg_bubble + css::conv_msg_bubble_user
        } else if status == MessageStatus::Failed {
            css::conv_msg_bubble + css::conv_msg_bubble_error
        } else {
            css::conv_msg_bubble + css::conv_msg_bubble_assistant
        }
    }

    let is_user = message.role == MessageRole::User;
    let mut reasoning_expanded = use_signal(|| false);

    let reasoning_text = resolve_reasoning_text(&streaming_reasoning, &message.reasoning_content);
    let has_reasoning = !reasoning_text.is_empty();

    let row_class = row_classes(is_user);
    let avatar_class = avatar_classes(is_user);
    let body_class = body_classes(is_user);
    let bubble_class = bubble_classes(is_user, message.status);

    rsx! {
        div {
            class: "{css::conv_message_canvas}",
            div {
                class: "{row_class}",
                div {
                    class: "{avatar_class}",
                    if is_user {
                        Icon { data: tabler::User, size: "24" }
                    } else {
                        Icon { data: tabler::Robot, size: "24" }
                    }
                }
                div {
                    class: "{body_class}",
                    div {
                        class: "{bubble_class} msg-content",
                        if has_reasoning {
                            div {
                                class: "{css::conv_msg_reasoning}",
                                div {
                                    class: "{css::conv_msg_reasoning_header}",
                                    onclick: move |_| reasoning_expanded.set(!reasoning_expanded()),
                                    span { {t!("chat.reasoning").to_string()} },
                                    Icon {
                                        data: if reasoning_expanded() { tabler::ChevronDown } else { tabler::ChevronRight },
                                        size: "14",
                                    }
                                }
                                if reasoning_expanded() {
                                    div {
                                        class: "{css::conv_msg_reasoning_body}",
                                        Markdown {
                                            content: reasoning_text.clone()
                                        }
                                    }
                                }
                            }
                        }
                        Markdown {
                            content: message.content.clone()
                        }
                    }
                    if message.status == MessageStatus::Truncated {
                        div {
                            class: "{css::conv_msg_truncated_hint}",
                            Icon { data: tabler::PlayerStop, size: "12" }
                            {t!("chat.status-truncated").to_string()}
                        }
                    }
                }
            }
        }
    }
}
