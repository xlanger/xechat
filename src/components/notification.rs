use dioxus::prelude::*;
use dioxus_style::with_css;
use std::time::Duration;
use crate::hooks::use_ui;
use crate::stores::ui::ToastKind;

/// 计算通知自动消失的延迟时间（毫秒）。
///
/// 若 toast 存在则使用其 `duration_ms`，否则默认 3000ms。
#[inline]
pub fn toast_duration(toast: Option<&crate::stores::ui::Toast>) -> u64 {
    toast.map(|t| t.duration_ms).unwrap_or(3000)
}

#[with_css(css, "styles/components/notification.scss")]
/// 通知组件，以 Toast 形式展示信息、成功或错误提示，支持自动消失。
#[component]
pub fn Notification() -> Element {
    /// 根据 ToastKind 返回 (toast_class, icon_class, icon_char) 三元组。
    fn toast_kind_styles(kind: ToastKind) -> (dioxus_style::CssClass, dioxus_style::CssClass, &'static str) {
        match kind {
            ToastKind::Info => (
                css::notification_toast_info,
                css::notification_icon_info,
                "\u{2139}",
            ),
            ToastKind::Success => (
                css::notification_toast_success,
                css::notification_icon_success,
                "\u{2713}",
            ),
            ToastKind::Error => (
                css::notification_toast_error,
                css::notification_icon_error,
                "\u{2715}",
            ),
        }
    }

    /// 获取可见性对应的 CSS 类名。
    fn visibility_class(visible: bool) -> dioxus_style::CssClass {
        if visible {
            css::notification_toast_visible
        } else {
            css::notification_toast_hidden
        }
    }

    let mut ui_store = use_ui();

    let mut visible = use_signal(|| false);

    use_effect(move || {
        if ui_store.active_toast.read().is_some() {
            visible.set(true);
            let duration = toast_duration(ui_store.active_toast.read().as_ref());
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

    let (toast_kind_class, icon_kind_class, icon) = toast_kind_styles(toast_data.kind);

    let vis_class = visibility_class(visible());

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
