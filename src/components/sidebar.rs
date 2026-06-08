use dioxus::prelude::*;
use dioxus_style::with_css;
use crate::hooks::{use_conversation, use_ui };
use crate::components::sidebar_header::SidebarHeader;
use crate::components::sidebar_footer::SidebarFooter;
use crate::components::conversation_item::ConversationItem;
use crate::stores::conversation::SIDEBAR_MAX_CONVERSATIONS;

#[with_css(css, "styles/components/sidebar.scss")]
/// 侧边栏组件，展示对话列表，包含头部和底部操作区。
#[component]
pub fn Sidebar() -> Element {
    let conv_store = use_conversation();
    let mut ui_store = use_ui();

    let close_menu = move |_| {
        ui_store.open_menu_id.set(None);
    };

    rsx! {
        div {
            class: "{css::sidebar}",
            onclick: close_menu,
            SidebarHeader {}
            div {
                class: "{css::sidebar_conv_list}",
                {
                    let conversations = conv_store.conversations.read();
                    let current_id = conv_store.current_conversation_id.read();
                    let non_temp: Vec<_> = conversations.iter().filter(|c| !c.is_temporary).collect();
                    let display: Vec<_> = non_temp.into_iter().take(SIDEBAR_MAX_CONVERSATIONS).collect();
                    rsx! {
                        for conv in display {
                            ConversationItem {
                                key: "{conv.id}",
                                conversation: conv.clone(),
                                is_active: current_id.as_ref() == Some(&conv.id),
                            }
                        }
                    }
                }
            }
            SidebarFooter {}
        }
    }
}
