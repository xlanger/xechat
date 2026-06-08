//! 对话列表项组件模块。
//!
//! 渲染侧边栏中的单个对话条目，支持选中高亮、
//! 悬停效果和右键菜单（重命名/删除）操作。
//! 本模块属于 components 层，通过 hooks 获取 store。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_conversation, use_ui, use_app};
use crate::state::MainRoute;
use crate::Conversation;
use crate::icons::{Icon, tabler};

#[with_css(css, "styles/components/conversation.scss")]
/// 对话列表项组件，支持选中、悬停和右键菜单（重命名/删除）操作。
#[component]
pub fn ConversationItem(
    /// 对话数据
    conversation: Conversation,
    /// 是否为当前选中状态
    is_active: bool,
) -> Element {
    let conv_store = use_conversation();
    let mut ui_store = use_ui();
    let app_store = use_app();
    let _lang = *app_store.language.read();
    let mut is_hovered = use_signal(|| false);
    let conv_id = conversation.id.clone();

    let is_menu_open = ui_store.open_menu_id.read().as_ref() == Some(&conv_id);

    let select_conv = {
        let conv_id = conv_id.clone();
        let mut conv_store = conv_store.clone();
        let mut app_store = app_store.clone();
        let mut ui_store = ui_store.clone();
        move |_| {
            let cid = conv_id.clone();
            conv_store.select_conversation(cid.clone());
            let cid_for_load = cid.clone();
            let mut load_store = conv_store.clone();
            spawn(async move {
                load_store.load_conversation_content(&cid_for_load).await;
            });
            ui_store.open_menu_id.set(None);
            ui_store.open_header_menu.set(false);
            app_store.navigate_to(MainRoute::Conversation(cid));
        }
    };

    let toggle_menu = {
        let conv_id = conv_id.clone();
        move |e: MouseEvent| {
            e.stop_propagation();
            let should_open = ui_store.open_menu_id.read().as_ref() != Some(&conv_id);
            if should_open {
                ui_store.open_menu_id.set(Some(conv_id.clone()));
            } else {
                ui_store.open_menu_id.set(None);
            }
        }
    };

    let rename_action = {
        let conv_id = conv_id.clone();
        move |_| {
            ui_store.open_menu_id.set(None);
            ui_store.show_rename_modal.set(Some(conv_id.clone()));
        }
    };

    let delete_action = {
        let conv_id = conv_id.clone();
        move |_| {
            ui_store.open_menu_id.set(None);
            ui_store.show_delete_modal.set(Some(conv_id.clone()));
        }
    };

    let rename_text = t!("menu.rename");
    let delete_text = t!("menu.delete");

    let hovered = *is_hovered.read();

    let inner_class = if is_active {
        css::conv_item_inner + css::conv_item_inner_active
    } else if hovered {
        css::conv_item_inner + css::conv_item_inner_hover
    } else {
        css::conv_item_inner
    };

    let title_class = if is_active {
        css::conv_item_title + css::conv_item_title_active
    } else if hovered {
        css::conv_item_title + css::conv_item_title_hover
    } else {
        css::conv_item_title
    };

    let more_btn_class = if is_menu_open || hovered {
        css::conv_item_more_btn + css::conv_item_more_btn_visible
    } else {
        css::conv_item_more_btn
    };

    rsx! {
        div {
            class: "{css::conv_item}",
            onclick: select_conv,
            onmouseenter: move |_| is_hovered.set(true),
            onmouseleave: move |_| is_hovered.set(false),
            div {
                class: "{inner_class}",
                div {
                    class: "{title_class}",
                    "{conversation.title}"
                }
                div {
                    class: "{more_btn_class}",
                    onmousedown: |e| e.stop_propagation(),
                    onclick: toggle_menu,
                    Icon { data: tabler::DotsVertical, size: "16" }
                }
                {
                    if is_menu_open {
                        rsx! {
                            div {
                                class: "{css::conv_item_menu}",
                                onclick: |e| e.stop_propagation(),
                                div {
                                    class: "{css::conv_item_menu_item}",
                                    onclick: rename_action,
                                    Icon { data: tabler::Pencil, size: "16" }
                                    span { "{rename_text}" }
                                }
                                div {
                                    class: "{css::conv_item_menu_item_danger}",
                                    onclick: delete_action,
                                    Icon { data: tabler::Trash, size: "16" }
                                    span { "{delete_text}" }
                                }
                            }
                        }
                    } else {
                        rsx! { {} }
                    }
                }
            }
        }
    }
}
