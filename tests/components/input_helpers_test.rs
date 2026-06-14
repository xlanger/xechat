use xechat::components::input::{
    handle_number_input, handle_number_blur,
};

// ── handle_number_input ───────────────────────────────────────────

#[test]
fn test_handle_number_input_valid_number() {
    let (clamped, valid, should_call) = handle_number_input("42", None, None);
    assert_eq!(clamped, "42");
    assert_eq!(valid, Some("42".to_string()));
    assert!(should_call);
}

#[test]
fn test_handle_number_input_empty() {
    let (clamped, valid, should_call) = handle_number_input("", None, None);
    assert_eq!(clamped, "");
    assert!(valid.is_none());
    assert!(!should_call);
}

#[test]
fn test_handle_number_input_with_clamp() {
    let (clamped, valid, should_call) = handle_number_input("150", Some(0.0), Some(100.0));
    assert_eq!(clamped, "100");
    assert_eq!(valid, Some("100".to_string()));
    assert!(should_call);
}

#[test]
fn test_handle_number_input_partial_number() {
    let (clamped, valid, should_call) = handle_number_input("1.", None, None);
    assert_eq!(clamped, "1.");
    // "1." parses as f64 successfully, so it IS a valid number
    assert_eq!(valid, Some("1.".to_string()));
    assert!(should_call);
}

#[test]
fn test_handle_number_input_negative() {
    let (clamped, valid, should_call) = handle_number_input("-5", None, None);
    assert_eq!(clamped, "-5");
    assert_eq!(valid, Some("-5".to_string()));
    assert!(should_call);
}

// ── handle_number_blur ────────────────────────────────────────────

#[test]
fn test_handle_number_blur_valid_unchanged() {
    let result = handle_number_blur("42", "42", None, None);
    assert!(result.is_none()); // no change needed
}

#[test]
fn test_handle_number_blur_empty_fallback() {
    let result = handle_number_blur("", "10", None, None);
    assert!(result.is_some());
    let (edit, valid) = result.unwrap();
    assert_eq!(edit, "10");
    assert_eq!(valid, "10");
}

#[test]
fn test_handle_number_blur_trailing_dot() {
    let result = handle_number_blur("1.", "1", None, None);
    assert!(result.is_some());
    let (edit, valid) = result.unwrap();
    assert_eq!(edit, "1");
    assert_eq!(valid, "1");
}

#[test]
fn test_handle_number_blur_invalid_fallback() {
    let result = handle_number_blur("abc", "5", None, None);
    // "abc" gets filtered to "" by filter_number_input, but handle_number_blur
    // checks is_valid_number which returns false for "abc"
    assert!(result.is_some());
}

#[test]
fn test_handle_number_blur_clamp_to_min() {
    let result = handle_number_blur("-5", "0", Some(0.0), Some(100.0));
    assert!(result.is_some());
    let (edit, valid) = result.unwrap();
    assert_eq!(edit, "0");
    assert_eq!(valid, "0");
}

#[test]
fn test_handle_number_blur_clamp_to_max() {
    let result = handle_number_blur("150", "100", Some(0.0), Some(100.0));
    assert!(result.is_some());
    let (edit, valid) = result.unwrap();
    assert_eq!(edit, "100");
    assert_eq!(valid, "100");
}

#[test]
fn test_handle_number_blur_empty_no_last_valid() {
    let result = handle_number_blur("", "0", None, None);
    // Falls back to "0", clamped to "0", same as current "" → should change
    assert!(result.is_some());
    let (edit, _valid) = result.unwrap();
    assert_eq!(edit, "0");
}
