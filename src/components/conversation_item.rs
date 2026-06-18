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

/// 判断菜单是否应显示（基于当前打开的菜单 ID 和对话 ID）。
pub fn is_menu_visible(open_menu_id: &Option<String>, conv_id: &str) -> bool {
    open_menu_id.as_ref() == Some(&conv_id.to_string())
}

/// 计算菜单切换后的目标菜单 ID。
///
/// 若当前对话的菜单已打开则返回 `None`（关闭），否则返回 `Some(conv_id)`（打开）。
#[inline]
pub fn next_menu_id(open_menu_id: &Option<String>, conv_id: &str) -> Option<String> {
    if open_menu_id.as_ref() == Some(&conv_id.to_string()) {
        None
    } else {
        Some(conv_id.to_string())
    }
}

/// 计算更多按钮的 CSS 类名。
fn item_more_btn_class(
    is_menu_open: bool,
    hovered: bool,
    base: dioxus_style::CssClass,
    visible: dioxus_style::CssClass,
) -> dioxus_style::CssClass {
    if is_menu_open || hovered {
        base + visible
    } else {
        base
    }
}

/// 渲染对话项的右键上下文菜单（重命名/删除）。
fn render_context_menu(
    is_menu_open: bool,
    menu_class: dioxus_style::CssClass,
    item_class: dioxus_style::CssClass,
    item_danger_class: dioxus_style::CssClass,
    rename_text: &str,
    delete_text: &str,
    on_rename: impl FnMut(MouseEvent) + 'static,
    on_delete: impl FnMut(MouseEvent) + 'static,
) -> Element {
    if is_menu_open {
        rsx! {
            div {
                class: "{menu_class}",
                onclick: |e| e.stop_propagation(),
                div {
                    class: "{item_class}",
                    onclick: on_rename,
                    Icon { data: tabler::Pencil, size: "16" }
                    span { "{rename_text}" }
                }
                div {
                    class: "{item_danger_class}",
                    onclick: on_delete,
                    Icon { data: tabler::Trash, size: "16" }
                    span { "{delete_text}" }
                }
            }
        }
    } else {
        rsx! { {} }
    }
}

#[with_css(css, "styles/components/conversation.scss")]
/// 对话列表项组件，支持选中、悬停和右键菜单（重命名/删除）操作。
#[component]
pub fn ConversationItem(
    /// 对话数据
    conversation: Conversation,
    /// 是否为当前选中状态
    is_active: bool,
) -> Element {
    /// 计算列表项内部容器的 CSS 类名。
    fn item_inner_class(is_active: bool, hovered: bool) -> dioxus_style::CssClass {
        if is_active {
            css::conv_item_inner + css::conv_item_inner_active
        } else if hovered {
            css::conv_item_inner + css::conv_item_inner_hover
        } else {
            css::conv_item_inner
        }
    }

    /// 计算列表项标题的 CSS 类名。
    fn item_title_class(is_active: bool, hovered: bool) -> dioxus_style::CssClass {
        if is_active {
            css::conv_item_title + css::conv_item_title_active
        } else if hovered {
            css::conv_item_title + css::conv_item_title_hover
        } else {
            css::conv_item_title
        }
    }

    let conv_store = use_conversation();
    let mut ui_store = use_ui();
    let app_store = use_app();
    let _lang = *app_store.language.read();
    let mut is_hovered = use_signal(|| false);
    let conv_id = conversation.id.clone();

    let is_menu_open = is_menu_visible(&ui_store.open_menu_id.read(), &conv_id);

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
            let next = next_menu_id(&ui_store.open_menu_id.read(), &conv_id);
            ui_store.open_menu_id.set(next);
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

    let i_class = item_inner_class(is_active, hovered);
    let t_class = item_title_class(is_active, hovered);
    let m_btn_class = item_more_btn_class(is_menu_open, hovered, css::conv_item_more_btn, css::conv_item_more_btn_visible);

    rsx! {
        div {
            class: "{css::conv_item}",
            onclick: select_conv,
            onmouseenter: move |_| is_hovered.set(true),
            onmouseleave: move |_| is_hovered.set(false),
            div {
                class: "{i_class}",
                div {
                    class: "{t_class}",
                    "{conversation.title}"
                }
                div {
                    class: "{m_btn_class}",
                    onmousedown: |e| e.stop_propagation(),
                    onclick: toggle_menu,
                    Icon { data: tabler::DotsVertical, size: "16" }
                }
                {render_context_menu(
                    is_menu_open,
                    css::conv_item_menu,
                    css::conv_item_menu_item,
                    css::conv_item_menu_item_danger,
                    &rename_text,
                    &delete_text,
                    rename_action,
                    delete_action,
                )}
            }
        }
    }
}
