use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_conversation, use_ui, use_app};
use crate::icons::{Icon, tabler};

#[with_css(css, "styles/components/conversation.scss")]
#[component]
pub fn ConversationHeader(
    title: String,
) -> Element {
    let mut ui_store = use_ui();
    let conv_store = use_conversation();
    let app_store = use_app();
    let _lang = *app_store.language.read();
    let mut is_hovered = use_signal(|| false);
    let is_menu_open = ui_store.open_header_menu.read();

    let mut toggle_menu = move |_e: MouseEvent| {
        let current = *ui_store.open_header_menu.read();
        ui_store.open_header_menu.set(!current);
    };

    let rename_action = {
        move |_| {
            if let Some(id) = conv_store.current_conversation_id.read().clone() {
                ui_store.open_header_menu.set(false);
                is_hovered.set(false);
                ui_store.show_rename_modal.set(Some(id));
            }
        }
    };

    let delete_action = {
        move |_| {
            if let Some(id) = conv_store.current_conversation_id.read().clone() {
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

    let header_wrapper_class = if hovered || menu_open {
        css::conv_header_title_wrapper + css::conv_header_title_wrapper_active
    } else {
        css::conv_header_title_wrapper
    };

    let title_class = if menu_open {
        css::conv_header_title + css::conv_header_title_active
    } else if hovered {
        css::conv_header_title + css::conv_header_title_hover
    } else {
        css::conv_header_title + css::conv_header_title_default
    };

    let arrow_class = if menu_open {
        css::conv_header_arrow + css::conv_header_arrow_open
    } else if hovered {
        css::conv_header_arrow + css::conv_header_arrow_hover
    } else {
        css::conv_header_arrow
    };

    rsx! {
        div {
            class: "{css::conv_header}",
            div {
                class: "{header_wrapper_class}",
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
                    class: "{title_class}",
                    "{title}"
                }
                span {
                    class: "{arrow_class}",
                    Icon { data: tabler::ChevronDown, size: "14" }
                }
                {
                    if *is_menu_open {
                            rsx! {
                                div {
                                    class: "{css::conv_header_dropdown}",
                                    onclick: |e| e.stop_propagation(),
                                div {
                                    class: "{css::conv_header_menu_item}",
                                    onclick: rename_action,
                                    Icon { data: tabler::Pencil, size: "16" }
                                    span { "{rename_text}" }
                                }
                                div {
                                    class: "{css::conv_header_menu_item_danger}",
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
