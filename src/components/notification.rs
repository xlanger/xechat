use dioxus::prelude::*;
use dioxus_style::with_css;
use std::time::Duration;
use crate::hooks::use_ui;
use crate::stores::ui::ToastKind;

#[with_css(css, "styles/components/notification.scss")]
/// 通知组件，以 Toast 形式展示信息、成功或错误提示，支持自动消失。
#[component]
pub fn Notification() -> Element {
    let mut ui_store = use_ui();

    let mut visible = use_signal(|| false);

    use_effect(move || {
        if ui_store.active_toast.read().is_some() {
            visible.set(true);
            let duration = ui_store.active_toast.read().as_ref().map(|t| t.duration_ms).unwrap_or(3000);
            spawn(async move {
                tokio::time::sleep(Duration::from_millis(duration)).await;
                visible.set(false);
                spawn(async move {
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    ui_store.active_toast.set(None);
                });
            });
        }
    });

    let toast_data = ui_store.active_toast.read();
    if toast_data.is_none() {
        return rsx! { {} };
    }
    let toast_data = toast_data.as_ref().unwrap();

    let toast_kind_class = match toast_data.kind {
        ToastKind::Info => css::notification_toast_info,
        ToastKind::Success => css::notification_toast_success,
        ToastKind::Error => css::notification_toast_error,
    };

    let icon_kind_class = match toast_data.kind {
        ToastKind::Info => css::notification_icon_info,
        ToastKind::Success => css::notification_icon_success,
        ToastKind::Error => css::notification_icon_error,
    };

    let vis_class = if visible() {
        css::notification_toast_visible
    } else {
        css::notification_toast_hidden
    };

    let icon = match toast_data.kind {
        ToastKind::Info => "\u{2139}",
        ToastKind::Success => "\u{2713}",
        ToastKind::Error => "\u{2715}",
    };

    let close = move |_| { ui_store.active_toast.set(None); };

    rsx! {
        div {
            class: "{css::notification_toast} {toast_kind_class} {vis_class}",
            span { class: "{css::notification_icon} {icon_kind_class}", "{icon}" }
            span { class: "{css::notification_message}", "{toast_data.message}" }
            span {
                class: "{css::notification_close}",
                onclick: close, "\u{2715}"
            }
        }
    }
}
