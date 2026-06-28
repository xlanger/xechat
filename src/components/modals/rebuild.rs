//! 重建向量确认模态框组件。
//!
//! 提示用户重建操作的影响，确认后执行向量重建。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_ui, use_conversation, use_app};
use crate::stores::ui::ToastKind;
use crate::components::modals::modal::{Modal, ModalFooter};

#[with_css(css, "styles/components/modals/modal.scss")]
#[component]
pub fn RebuildModal() -> Element {
    let mut ui_store = use_ui();
    let app_store = use_app();
    let _lang = *app_store.language.read();

    let show = *ui_store.show_rebuild_modal.read();

    let title = t!("modal.rebuild-title");
    let message = t!("modal.rebuild-message");
    let cancel = t!("modal.cancel");
    let confirm = t!("modal.rebuild-confirm");

    rsx! {
        Modal {
            show,
            title,
            onclose: move |_| ui_store.show_rebuild_modal.set(false),
            p {
                class: "{css::delete_message}",
                "{message}"
            }
            ModalFooter {
                div {
                    class: "{css::modal_btn_cancel} btn-secondary",
                    onclick: move |_| ui_store.show_rebuild_modal.set(false),
                    "{cancel}"
                }
                div {
                    class: "{css::modal_btn_confirm} btn-primary",
                    onclick: {
                        let ui_store = ui_store;
                        move |_| {
                            let mut ui_store = ui_store;
                            spawn(async move {
                                let mut conv = use_conversation();
                                conv.rebuild_vectors().await;
                                ui_store.show_rebuild_modal.set(false);
                                let msg = t!("toast.turns-rebuilt").to_string();
                                ui_store.show_toast(ToastKind::Info, msg, 5000);
                            });
                        }
                    },
                    "{confirm}"
                }
            }
        }
    }
}
