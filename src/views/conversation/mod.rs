//! 对话视图模块，实现对话页面的核心交互

pub mod header;
pub mod message_list;
pub mod message_bubble;
pub mod chat_input;

pub use header::ConversationHeader;
pub use message_list::MessageList;
pub use message_bubble::MessageBubble;
pub use chat_input::ChatInput;

use dioxus::prelude::*;
use dioxus_style::with_css;
use crate::hooks::{use_conversation, use_app};

#[with_css(css, "styles/components/conversation.scss")]
#[component]
pub fn ConversationView() -> Element {
    let conv_store = use_conversation();
    let _app_store = use_app();

    let current_conv: Memo<Option<crate::Conversation>> = use_memo(move || {
        let current_id = conv_store.current_conversation_id.read();
        let id = match current_id.as_ref() {
            Some(id) => id.clone(),
            None => return None,
        };
        let convs = conv_store.conversations.read();
        convs.iter().find(|c| c.id == id).cloned()
    });

    let conv = match current_conv.read().as_ref() {
        Some(c) => c.clone(),
        None => return rsx! { {} },
    };

    let title = conv.title.clone();

    rsx! {
        div {
            class: "{css::conv_screen}",
            ConversationHeader {
                title,
            }
            MessageList {}
            ChatInput {}
        }
    }
}
