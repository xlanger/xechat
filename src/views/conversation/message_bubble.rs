use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::MessageRole;
use crate::MessageStatus;
use crate::icons::{Icon, tabler};
use crate::components::markdown::Markdown;

#[with_css(css, "styles/components/conversation.scss")]
#[component]
pub fn MessageBubble(
    message: crate::Message,
    /// 流式推理过程（仅流式气泡传入，持久化消息使用 message.reasoning_content）
    streaming_reasoning: Option<String>,
) -> Element {
    let is_user = message.role == MessageRole::User;
    let mut reasoning_expanded = use_signal(|| false);

    // 优先使用传入的流式推理内容，否则使用消息自身的推理内容
    let reasoning_text = streaming_reasoning.unwrap_or_else(|| message.reasoning_content.clone().unwrap_or_default());
    let has_reasoning = !reasoning_text.is_empty();

    let row_class = if is_user {
        css::conv_msg_row + css::conv_msg_row_user
    } else {
        css::conv_msg_row
    };

    let avatar_class = if is_user {
        css::conv_msg_avatar + css::conv_msg_avatar_user
    } else {
        css::conv_msg_avatar + css::conv_msg_avatar_assistant
    };

    let body_class = if is_user {
        css::conv_msg_body + css::conv_msg_body_user
    } else {
        css::conv_msg_body + css::conv_msg_body_assistant
    };

    let bubble_class = if is_user {
        css::conv_msg_bubble + css::conv_msg_bubble_user
    } else if message.status == MessageStatus::Failed {
        css::conv_msg_bubble + css::conv_msg_bubble_error
    } else {
        css::conv_msg_bubble + css::conv_msg_bubble_assistant
    };

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
