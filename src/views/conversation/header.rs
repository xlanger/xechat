//! 对话头部组件模块。
//!
//! 显示当前对话标题，支持悬停效果和下拉菜单（重命名/删除）操作。
//! 本模块属于 views 层，通过 hooks 获取 store。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_conversation, use_ui, use_app};
use crate::icons::{Icon, tabler};

/// 切换头部菜单的打开状态，返回取反后的布尔值。
#[inline]
pub fn toggle_header_menu_state(is_open: bool) -> bool {
    !is_open
}

/// 构建菜单操作的目标对话 ID。
///
/// 若存在当前对话 ID，返回 `Some(id)`；否则返回 `None`。
#[inline]
pub fn current_conv_id_for_action(current_id: &Option<String>) -> Option<String> {
    current_id.clone()
}

/// 计算标题包装器的 CSS 类名。
fn header_wrapper_class(
    hovered: bool,
    menu_open: bool,
    base: dioxus_style::CssClass,
    active: dioxus_style::CssClass,
) -> dioxus_style::CssClass {
    if hovered || menu_open {
        base + active
    } else {
        base
    }
}

/// 渲染对话头部的下拉菜单（重命名/删除）。
fn render_header_actions(
    is_menu_open: bool,
    dropdown_class: dioxus_style::CssClass,
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
                class: "{dropdown_class}",
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
/// 对话头部组件，显示标题和下拉菜单。
#[component]
pub fn ConversationHeader(
    /// 对话标题
    title: String,
) -> Element {
    /// 计算标题文本的 CSS 类名。
    fn header_title_class(hovered: bool, menu_open: bool) -> dioxus_style::CssClass {
        if menu_open {
            css::conv_header_title + css::conv_header_title_active
        } else if hovered {
            css::conv_header_title + css::conv_header_title_hover
        } else {
            css::conv_header_title + css::conv_header_title_default
        }
    }

    /// 计算箭头的 CSS 类名。
    fn header_arrow_class(hovered: bool, menu_open: bool) -> dioxus_style::CssClass {
        if menu_open {
            css::conv_header_arrow + css::conv_header_arrow_open
        } else if hovered {
            css::conv_header_arrow + css::conv_header_arrow_hover
        } else {
            css::conv_header_arrow
        }
    }

    let mut ui_store = use_ui();
    let conv_store = use_conversation();
    let app_store = use_app();
    let _lang = *app_store.language.read();
    let mut is_hovered = use_signal(|| false);
    let is_menu_open = ui_store.open_header_menu.read();

    let mut toggle_menu = move |_e: MouseEvent| {
        let current = *ui_store.open_header_menu.read();
        ui_store.open_header_menu.set(toggle_header_menu_state(current));
    };

    let rename_action = {
        move |_| {
            if let Some(id) = current_conv_id_for_action(&conv_store.current_conversation_id.read().clone()) {
                ui_store.open_header_menu.set(false);
                is_hovered.set(false);
                ui_store.show_rename_modal.set(Some(id));
            }
        }
    };

    let delete_action = {
        move |_| {
            if let Some(id) = current_conv_id_for_action(&conv_store.current_conversation_id.read().clone()) {
                ui_store.open_header_menu.set(false);
                is_hovered.set(false);
                ui_store.show_delete_modal.set(Some(id));
            }
        }
    };

    let rename_text = t!("menu.rename");
    let delete_text = t!("menu.delete");

    let hovered = *is_hovered.read();
    let menu_open = *is_menu_open;

    let h_wrapper_class = header_wrapper_class(hovered, menu_open, css::conv_header_title_wrapper, css::conv_header_title_wrapper_active);
    let h_title_class = header_title_class(hovered, menu_open);
    let h_arrow_class = header_arrow_class(hovered, menu_open);

    rsx! {
        div {
            class: "{css::conv_header}",
            div {
                class: "{h_wrapper_class}",
                onmouseenter: move |_| is_hovered.set(true),
                onmouseleave: move |_| {
                    is_hovered.set(false);
                },
                onmousedown: |evt| { evt.stop_propagation(); },
                onclick: move |evt| {
                    evt.stop_propagation();
                    toggle_menu(evt);
                },
                div {
                    class: "{h_title_class}",
                    "{title}"
                }
                span {
                    class: "{h_arrow_class}",
                    Icon { data: tabler::ChevronDown, size: "14" }
                }
                {render_header_actions(
                    menu_open,
                    css::conv_header_dropdown,
                    css::conv_header_menu_item,
                    css::conv_header_menu_item_danger,
                    &rename_text,
                    &delete_text,
                    rename_action,
                    delete_action,
                )}
            }
        }
    }
}
