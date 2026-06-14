use xechat::components::conversation_item::{is_menu_visible, next_menu_id};

// ── is_menu_visible ─────────────────────────────────────────────

#[test]
fn test_is_menu_visible_matching_id() {
    assert!(is_menu_visible(&Some("conv-1".to_string()), "conv-1"));
}

#[test]
fn test_is_menu_visible_non_matching_id() {
    assert!(!is_menu_visible(&Some("conv-1".to_string()), "conv-2"));
}

#[test]
fn test_is_menu_visible_none_open() {
    assert!(!is_menu_visible(&None, "conv-1"));
}

#[test]
fn test_is_menu_visible_empty_string_id() {
    assert!(is_menu_visible(&Some("".to_string()), ""));
    assert!(!is_menu_visible(&Some("".to_string()), "conv-1"));
}

// ── next_menu_id ────────────────────────────────────────────────

#[test]
fn test_next_menu_id_when_menu_closed() {
    assert_eq!(next_menu_id(&None, "conv-1"), Some("conv-1".to_string()));
}

#[test]
fn test_next_menu_id_when_other_menu_open() {
    assert_eq!(next_menu_id(&Some("conv-2".to_string()), "conv-1"), Some("conv-1".to_string()));
}

#[test]
fn test_next_menu_id_when_same_menu_open() {
    assert_eq!(next_menu_id(&Some("conv-1".to_string()), "conv-1"), None);
}

#[test]
fn test_next_menu_id_empty_conv_id() {
    assert_eq!(next_menu_id(&None, ""), Some("".to_string()));
    assert_eq!(next_menu_id(&Some("".to_string()), ""), None);
}
