use xechat::views::conversation::header::{toggle_header_menu_state, current_conv_id_for_action};

// ── toggle_header_menu_state ────────────────────────────────────

#[test]
fn test_toggle_header_menu_state_open_to_closed() {
    assert!(!toggle_header_menu_state(true));
}

#[test]
fn test_toggle_header_menu_state_closed_to_open() {
    assert!(toggle_header_menu_state(false));
}

// ── current_conv_id_for_action ──────────────────────────────────

#[test]
fn test_current_conv_id_for_action_some() {
    let id = Some("conv-123".to_string());
    assert_eq!(current_conv_id_for_action(&id), Some("conv-123".to_string()));
}

#[test]
fn test_current_conv_id_for_action_none() {
    assert_eq!(current_conv_id_for_action(&None), None);
}

#[test]
fn test_current_conv_id_for_action_empty_string() {
    let id = Some("".to_string());
    assert_eq!(current_conv_id_for_action(&id), Some("".to_string()));
}
