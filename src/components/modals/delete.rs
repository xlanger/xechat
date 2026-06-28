use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_ui, use_conversation, use_app};
use crate::stores::ui::{Toast, ToastKind};
use crate::components::modals::modal::{Modal, ModalFooter};

/// 删除会话确认模态框组件。
///
/// 显示删除警告文案，用户确认后删除指定会话。
/// 若被删的是当前活跃会话则清除活跃状态，成功删除后弹出 Toast 通知。
#[with_css(css, "styles/components/modals/modal.scss")]
#[component]
pub fn DeleteModal() -> Element {
    let mut ui_store = use_ui();
    let _conv_store = use_conversation();
    let app_store = use_app();
    let _lang = *app_store.language.read(); // 触发响应式更新

    let conv_id = ui_store.show_delete_modal.read().clone();

    let show = conv_id.is_some();
    let conv_id = match conv_id {
        Some(id) => id,
        None => return rsx! { {} },
    };

    let title = t!("modal.delete-title");
    let message = t!("modal.delete-message");
    let cancel = t!("modal.cancel");
    let confirm = t!("modal.delete-confirm");

    rsx! {
        Modal {
            show,
            title,
            onclose: move |_| ui_store.show_delete_modal.set(None),
            p {
                class: "{css::delete_message}",
                "{message}"
            }
            ModalFooter {
                div {
                    class: "{css::modal_btn_cancel} btn-secondary",
                    onclick: move |_| ui_store.show_delete_modal.set(None),
                    "{cancel}"
                }
                div {
                    class: "{css::modal_btn_confirm} btn-danger",
                    onclick: {
                        let id = conv_id.clone();
                        let conv_store = _conv_store.clone();
                        let ui_store = ui_store;
                        move |_| {
                            let id = id.clone();
                            let mut conv_store = conv_store.clone();
                            let mut ui_store = ui_store;
                            spawn(async move {
                                if conv_store.delete_conversation(&id).await.is_ok() {
                                    ui_store.show_delete_modal.set(None);
                                    ui_store.active_toast.set(Some(Toast {
                                        message: t!("toast.conversation-deleted").into_owned(),
                                        kind: ToastKind::Success,
                                        duration_ms: 3000,
                                    }));
                                }
                            });
                        }
                    },
                    "{confirm}"
                }
            }
        }
    }
}
