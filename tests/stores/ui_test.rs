use dioxus::prelude::*;
use dioxus_core::{NoOpMutations, Runtime, RuntimeGuard};
use xechat::stores::ui::{Toast, ToastKind, UIStore};

fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let mut vdom = VirtualDom::new(|| rsx! { div {} });
    vdom.rebuild(&mut NoOpMutations);
    vdom.in_runtime(|| {
        let runtime = Runtime::current();
        let _guard = RuntimeGuard::new(runtime.clone());
        runtime.in_scope(ScopeId::APP, f)
    })
}

#[test]
fn test_ui_store_new_default_values() {
    with_runtime(|| {
        let store = UIStore::new();

        assert!(!(store.show_config_modal)());
        assert!((store.show_rename_modal)().is_none());
        assert!((store.show_delete_modal)().is_none());
        assert!((store.open_menu_id)().is_none());
        assert!(!(store.open_header_menu)());
        assert!((store.active_toast)().is_none());
        assert!((store.menu_position)().is_none());
    });
}

#[test]
fn test_show_toast_sets_active_toast() {
    with_runtime(|| {
        let mut store = UIStore::new();

        store.show_toast(ToastKind::Error, "test error".to_string(), 3000);

        let toast = (store.active_toast)().expect("toast should be set");
        assert_eq!(toast.message, "test error");
        assert_eq!(toast.kind, ToastKind::Error);
        assert_eq!(toast.duration_ms, 3000);
    });
}

#[test]
fn test_show_toast_info_kind() {
    with_runtime(|| {
        let mut store = UIStore::new();

        store.show_toast(ToastKind::Info, "info msg".to_string(), 1000);

        let toast = (store.active_toast)().expect("toast should be set");
        assert_eq!(toast.kind, ToastKind::Info);
        assert_eq!(toast.message, "info msg");
        assert_eq!(toast.duration_ms, 1000);
    });
}

#[test]
fn test_show_toast_success_kind() {
    with_runtime(|| {
        let mut store = UIStore::new();

        store.show_toast(ToastKind::Success, "ok".to_string(), 2000);

        let toast = (store.active_toast)().expect("toast should be set");
        assert_eq!(toast.kind, ToastKind::Success);
    });
}

#[test]
fn test_hide_toast_clears_active_toast() {
    with_runtime(|| {
        let mut store = UIStore::new();

        store.show_toast(ToastKind::Info, "visible".to_string(), 1000);
        assert!((store.active_toast)().is_some());

        store.hide_toast();
        assert!((store.active_toast)().is_none());
    });
}

#[test]
fn test_show_toast_replaces_previous() {
    with_runtime(|| {
        let mut store = UIStore::new();

        store.show_toast(ToastKind::Info, "first".to_string(), 1000);
        store.show_toast(ToastKind::Error, "second".to_string(), 2000);

        let toast = (store.active_toast)().expect("toast should be set");
        assert_eq!(toast.message, "second");
        assert_eq!(toast.kind, ToastKind::Error);
    });
}

#[test]
fn test_toast_kind_equality() {
    assert_eq!(ToastKind::Info, ToastKind::Info);
    assert_eq!(ToastKind::Success, ToastKind::Success);
    assert_eq!(ToastKind::Error, ToastKind::Error);
    assert_ne!(ToastKind::Info, ToastKind::Error);
    assert_ne!(ToastKind::Success, ToastKind::Error);
}

#[test]
fn test_toast_kind_copy() {
    let kind = ToastKind::Info;
    let copied = kind;
    assert_eq!(kind, copied);
}

#[test]
fn test_toast_struct_clone() {
    let toast = Toast {
        message: "hello".to_string(),
        kind: ToastKind::Success,
        duration_ms: 5000,
    };
    let cloned = toast.clone();
    assert_eq!(toast.message, cloned.message);
    assert_eq!(toast.kind, cloned.kind);
    assert_eq!(toast.duration_ms, cloned.duration_ms);
}

#[test]
fn test_show_config_modal_signal() {
    with_runtime(|| {
        let mut store = UIStore::new();
        assert!(!(store.show_config_modal)());

        store.show_config_modal.set(true);
        assert!((store.show_config_modal)());
    });
}

#[test]
fn test_show_rename_modal_signal() {
    with_runtime(|| {
        let mut store = UIStore::new();
        assert!((store.show_rename_modal)().is_none());

        store.show_rename_modal.set(Some("conv-123".to_string()));
        let val = (store.show_rename_modal)();
        assert_eq!(val.as_deref(), Some("conv-123"));
    });
}

#[test]
fn test_show_delete_modal_signal() {
    with_runtime(|| {
        let mut store = UIStore::new();
        assert!((store.show_delete_modal)().is_none());

        store.show_delete_modal.set(Some("conv-456".to_string()));
        let val = (store.show_delete_modal)();
        assert_eq!(val.as_deref(), Some("conv-456"));
    });
}

#[test]
fn test_open_menu_id_signal() {
    with_runtime(|| {
        let mut store = UIStore::new();
        assert!((store.open_menu_id)().is_none());

        store.open_menu_id.set(Some("menu-1".to_string()));
        let val = (store.open_menu_id)();
        assert_eq!(val.as_deref(), Some("menu-1"));
    });
}

#[test]
fn test_open_header_menu_signal() {
    with_runtime(|| {
        let mut store = UIStore::new();
        assert!(!(store.open_header_menu)());

        store.open_header_menu.set(true);
        assert!((store.open_header_menu)());
    });
}

#[test]
fn test_menu_position_signal() {
    with_runtime(|| {
        let mut store = UIStore::new();
        assert!((store.menu_position)().is_none());

        store.menu_position.set(Some((100.0, 200.0)));
        let val = (store.menu_position)();
        assert_eq!(val, Some((100.0, 200.0)));
    });
}
