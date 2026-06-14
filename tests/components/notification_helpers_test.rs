use xechat::components::notification::toast_duration;
use xechat::stores::ui::{Toast, ToastKind};

// ── toast_duration ──────────────────────────────────────────────

#[test]
fn test_toast_duration_none() {
    assert_eq!(toast_duration(None), 3000);
}

#[test]
fn test_toast_duration_with_custom_duration() {
    let toast = Toast {
        kind: ToastKind::Info,
        message: "test".to_string(),
        duration_ms: 5000,
    };
    assert_eq!(toast_duration(Some(&toast)), 5000);
}

#[test]
fn test_toast_duration_with_zero_duration() {
    let toast = Toast {
        kind: ToastKind::Error,
        message: "test".to_string(),
        duration_ms: 0,
    };
    assert_eq!(toast_duration(Some(&toast)), 0);
}

#[test]
fn test_toast_duration_with_default_duration() {
    let toast = Toast {
        kind: ToastKind::Success,
        message: "test".to_string(),
        duration_ms: 3000,
    };
    assert_eq!(toast_duration(Some(&toast)), 3000);
}
