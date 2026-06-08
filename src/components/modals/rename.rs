use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_ui, use_conversation, use_app};
use crate::stores::ui::{Toast, ToastKind};
use crate::components::modals::modal::{Modal, ModalFooter};

/// 重命名会话模态框组件。
///
/// 显示输入框供用户修改会话标题，支持回车快捷确认。
/// 保存成功后刷新会话列表并弹出 Toast 通知。
#[with_css(css, "styles/components/modals/modal.scss")]
#[component]
pub fn RenameModal() -> Element {
    let mut ui_store = use_ui();
    let conv_store = use_conversation();
    let app_store = use_app();
    let _lang = *app_store.language.read(); // 触发响应式更新

    let conv_id = ui_store.show_rename_modal.read().clone();

    let show = conv_id.is_some();
    let conv_id = match conv_id {
        Some(id) => id,
        None => return rsx! { {} },
    };

    let current_title = conv_store.conversations.read()
        .iter().find(|c| c.id == conv_id).map(|c| c.title.clone()).unwrap_or_default();

    let mut input_value = use_signal(|| current_title.clone());

    let title = t!("modal.rename-title");
    let placeholder = t!("modal.rename-placeholder");
    let cancel = t!("modal.cancel");
    let confirm = t!("modal.confirm");

    rsx! {
        Modal {
            show,
            title,
            onclose: move |_| ui_store.show_rename_modal.set(None),
            input {
                class: "{css::rename_input}",
                placeholder: "{placeholder}",
                value: "{input_value}",
                oninput: move |e| input_value.set(e.value()),
                onkeydown: {
                    let id = conv_id.clone();
                    let new_title = input_value.read().clone();
                    let conv_store = conv_store.clone();
                    let ui_store = ui_store.clone();
                    move |evt| {
                        if evt.key() == Key::Enter && !new_title.trim().is_empty() {
                            evt.prevent_default();
                            let id = id.clone();
                            let nt = new_title.trim().to_string();
                            let mut conv_store = conv_store.clone();
                            let mut ui_store = ui_store.clone();
                            spawn(async move {
                                if conv_store.rename_conversation(&id, &nt).await.is_ok() {
                                    ui_store.show_rename_modal.set(None);
                                    ui_store.active_toast.set(Some(Toast {
                                        message: t!("toast.conversation-renamed").into_owned(),
                                        kind: ToastKind::Success,
                                        duration_ms: 3000,
                                    }));
                                }
                            });
                        }
                    }
                },
            }
            ModalFooter {
                div {
                    class: "{css::modal_btn_cancel} btn-secondary",
                    onclick: move |_| ui_store.show_rename_modal.set(None),
                    "{cancel}"
                }
                div {
                    class: "{css::modal_btn_confirm} btn-primary",
                    onclick: {
                        let id = conv_id.clone();
                        let nt = input_value.read().trim().to_string();
                        let conv_store = conv_store.clone();
                        let ui_store = ui_store.clone();
                        move |_| {
                            if nt.is_empty() { return; }
                            let id = id.clone();
                            let nt = nt.clone();
                            let mut conv_store = conv_store.clone();
                            let mut ui_store = ui_store.clone();
                            spawn(async move {
                                if conv_store.rename_conversation(&id, &nt).await.is_ok() {
                                    ui_store.show_rename_modal.set(None);
                                    ui_store.active_toast.set(Some(Toast {
                                        message: t!("toast.conversation-renamed").into_owned(),
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
