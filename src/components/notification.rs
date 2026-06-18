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

/// 调度通知自动消失定时器，先等待 duration_ms 后隐藏，再等待 400ms 后清除 toast。
fn compute_notification_duration(
    duration: u64,
    mut visible: Signal<bool>,
    mut active_toast: Signal<Option<crate::stores::ui::Toast>>,
) {
    spawn(async move {
        tokio::time::sleep(Duration::from_millis(duration)).await;
        visible.set(false);
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            active_toast.set(None);
        });
    });
}

/// 渲染通知主体内容（图标 + 消息 + 关闭按钮）。
fn render_notification_body(
    toast_data: &crate::stores::ui::Toast,
    visible: bool,
    toast_base_class: dioxus_style::CssClass,
    info_toast_class: dioxus_style::CssClass,
    success_toast_class: dioxus_style::CssClass,
    error_toast_class: dioxus_style::CssClass,
    icon_base_class: dioxus_style::CssClass,
    info_icon_class: dioxus_style::CssClass,
    success_icon_class: dioxus_style::CssClass,
    error_icon_class: dioxus_style::CssClass,
    message_class: dioxus_style::CssClass,
    close_class: dioxus_style::CssClass,
    visible_class: dioxus_style::CssClass,
    hidden_class: dioxus_style::CssClass,
    on_close: impl FnMut(MouseEvent) + 'static,
) -> Element {
    let (toast_kind_class, icon_kind_class, icon) = match toast_data.kind {
        ToastKind::Info => (info_toast_class, info_icon_class, "\u{2139}"),
        ToastKind::Success => (success_toast_class, success_icon_class, "\u{2713}"),
        ToastKind::Error => (error_toast_class, error_icon_class, "\u{2715}"),
    };
    let vis_class = if visible { visible_class } else { hidden_class };
    rsx! {
        div {
            class: "{toast_base_class} {toast_kind_class} {vis_class}",
            span { class: "{icon_base_class} {icon_kind_class}", "{icon}" }
            span { class: "{message_class}", "{toast_data.message}" }
            span {
                class: "{close_class}",
                onclick: on_close, "\u{2715}"
            }
        }
    }
}

#[with_css(css, "styles/components/notification.scss")]
/// 通知组件，以 Toast 形式展示信息、成功或错误提示，支持自动消失。
#[component]
pub fn Notification() -> Element {
    let mut ui_store = use_ui();

    let mut visible = use_signal(|| false);

    use_effect(move || {
        if ui_store.active_toast.read().is_some() {
            visible.set(true);
            let duration = toast_duration(ui_store.active_toast.read().as_ref());
            compute_notification_duration(duration, visible, ui_store.active_toast);
        }
    });

    let toast_data = ui_store.active_toast.read();
    if toast_data.is_none() {
        return rsx! { {} };
    }
    let toast_data = toast_data.as_ref().unwrap();

    let close = move |_| { ui_store.active_toast.set(None); };

    render_notification_body(
        toast_data,
        visible(),
        css::notification_toast,
        css::notification_toast_info,
        css::notification_toast_success,
        css::notification_toast_error,
        css::notification_icon,
        css::notification_icon_info,
        css::notification_icon_success,
        css::notification_icon_error,
        css::notification_message,
        css::notification_close,
        css::notification_toast_visible,
        css::notification_toast_hidden,
        close,
    )
}
