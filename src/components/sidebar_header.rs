use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_app, use_conversation};
use crate::state::MainRoute;
use crate::icons::{Icon, tabler};

#[with_css(css, "styles/components/sidebar.scss")]
/// 侧边栏头部组件，展示品牌标识和新建对话按钮。
#[component]
pub fn SidebarHeader() -> Element {
    let app_store = use_app();
    let _lang = *app_store.language.read(); // 触发响应式更新

    let new_chat_text = t!("sidebar.new-chat");

    let conv_store = use_conversation();
    let app_store = use_app();

    let onclick = {
        let mut app_store = app_store.clone();
        let mut conv_store = conv_store.clone();
        move |_| {
            conv_store.current_conversation_id.set(None);
            app_store.navigate_to(MainRoute::Welcome);
        }
    };

    rsx! {
        div {
            class: "{css::sidebar_header}",
            div {
                class: "{css::sidebar_header_brand}",
                img {
                    src: "{crate::assets::logo_data_url()}",
                    class: "{css::sidebar_header_logo}",
                    alt: "Logo",
                }
                h1 { class: "brand-text sidebar-brand", "XEChat" }
            }
            div {
                class: "{css::sidebar_header_new_chat}",
                onclick,
                div {
                    class: "{css::sidebar_header_new_chat_left}",
                    span {
                        class: "{css::sidebar_header_new_chat_icon}",
                        Icon { data: tabler::MessageChatbot, size: "20" }
                    }
                    "{new_chat_text}"
                }
                span {
                    class: "{css::sidebar_header_shortcut}",
                    span {
                        class: "{css::sidebar_header_kbd}",
                        Icon { data: tabler::Command, size: "12" }
                    }
                    span {
                        class: "{css::sidebar_header_kbd}",
                        Icon { data: tabler::LetterK, size: "12" }
                    }
                }
            }
        }
    }
}
