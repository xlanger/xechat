use xechat::state::{ThemeMode, MainRoute, Toast, ToastKind};

#[test]
fn test_theme_mode_system() {
    let mode = ThemeMode::System;
    assert_eq!(mode, ThemeMode::System);
}

#[test]
fn test_theme_mode_light() {
    let mode = ThemeMode::Light;
    assert_eq!(mode, ThemeMode::Light);
}

#[test]
fn test_theme_mode_dark() {
    let mode = ThemeMode::Dark;
    assert_eq!(mode, ThemeMode::Dark);
}

#[test]
fn test_theme_mode_inequality() {
    assert_ne!(ThemeMode::System, ThemeMode::Light);
    assert_ne!(ThemeMode::System, ThemeMode::Dark);
    assert_ne!(ThemeMode::Light, ThemeMode::Dark);
}

#[test]
fn test_theme_mode_copy() {
    let mode = ThemeMode::Dark;
    let copied = mode;
    assert_eq!(mode, copied);
}

#[test]
fn test_theme_mode_clone() {
    let mode = ThemeMode::Light;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_main_route_welcome() {
    let route = MainRoute::Welcome;
    assert_eq!(route, MainRoute::Welcome);
}

#[test]
fn test_main_route_settings() {
    let route = MainRoute::Settings;
    assert_eq!(route, MainRoute::Settings);
}

#[test]
fn test_main_route_conversation_with_id() {
    let route = MainRoute::Conversation("conv-123".to_string());
    match route {
        MainRoute::Conversation(id) => assert_eq!(id, "conv-123"),
        _ => panic!("expected Conversation variant"),
    }
}

#[test]
fn test_main_route_conversation_equality() {
    let a = MainRoute::Conversation("abc".to_string());
    let b = MainRoute::Conversation("abc".to_string());
    assert_eq!(a, b);
}

#[test]
fn test_main_route_conversation_inequality() {
    let a = MainRoute::Conversation("abc".to_string());
    let b = MainRoute::Conversation("xyz".to_string());
    assert_ne!(a, b);
}

#[test]
fn test_main_route_different_variants_inequality() {
    assert_ne!(MainRoute::Welcome, MainRoute::Settings);
    assert_ne!(MainRoute::Welcome, MainRoute::Conversation("x".to_string()));
    assert_ne!(MainRoute::Settings, MainRoute::Conversation("x".to_string()));
}

#[test]
fn test_main_route_default() {
    let default: MainRoute = MainRoute::default();
    assert_eq!(default, MainRoute::Welcome);
}

#[test]
fn test_main_route_clone() {
    let route = MainRoute::Conversation("test-id".to_string());
    let cloned = route.clone();
    assert_eq!(route, cloned);
}

#[test]
fn test_toast_construction() {
    let toast = Toast {
        message: "Saved".to_string(),
        kind: ToastKind::Success,
        duration_ms: 3000,
    };
    assert_eq!(toast.message, "Saved");
    assert!(toast.kind == ToastKind::Success);
    assert_eq!(toast.duration_ms, 3000);
}

#[test]
fn test_toast_equality() {
    let a = Toast {
        message: "Error".to_string(),
        kind: ToastKind::Error,
        duration_ms: 5000,
    };
    let b = Toast {
        message: "Error".to_string(),
        kind: ToastKind::Error,
        duration_ms: 5000,
    };
    assert!(a == b);
}

#[test]
fn test_toast_inequality() {
    let a = Toast {
        message: "OK".to_string(),
        kind: ToastKind::Success,
        duration_ms: 3000,
    };
    let b = Toast {
        message: "OK".to_string(),
        kind: ToastKind::Info,
        duration_ms: 3000,
    };
    assert!(a != b);
}

#[test]
fn test_toast_kind_variants() {
    assert!(ToastKind::Info == ToastKind::Info);
    assert!(ToastKind::Success == ToastKind::Success);
    assert!(ToastKind::Error == ToastKind::Error);
    assert!(ToastKind::Info != ToastKind::Success);
    assert!(ToastKind::Success != ToastKind::Error);
}

#[test]
fn test_toast_clone() {
    let toast = Toast {
        message: "test".to_string(),
        kind: ToastKind::Info,
        duration_ms: 1000,
    };
    let cloned = toast.clone();
    assert_eq!(toast.message, cloned.message);
    assert!(toast.kind == cloned.kind);
    assert_eq!(toast.duration_ms, cloned.duration_ms);
}
