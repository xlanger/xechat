use xechat::components::input::{
    clamp_number_value, filter_number_input, is_valid_number, normalize_number,
};

#[test]
fn test_filter_number_input() {
    assert_eq!(filter_number_input("abc"), "");
    assert_eq!(filter_number_input("123"), "123");
    assert_eq!(filter_number_input("0.1"), "0.1");
    assert_eq!(filter_number_input("1.2.3"), "1.23");
    assert_eq!(filter_number_input("-1.5"), "-1.5");
    assert_eq!(filter_number_input("1-5"), "15");
    assert_eq!(filter_number_input("--1"), "-1");
    assert_eq!(filter_number_input(".5"), ".5");
    assert_eq!(filter_number_input("5."), "5.");
    assert_eq!(filter_number_input(""), "");
}

#[test]
fn test_is_valid_number() {
    assert!(is_valid_number("1"));
    assert!(is_valid_number("1.5"));
    assert!(is_valid_number("0.1"));
    assert!(is_valid_number("1."));
    assert!(is_valid_number("-1.5"));
    assert!(!is_valid_number(""));
    assert!(!is_valid_number("-"));
    assert!(!is_valid_number("."));
    assert!(!is_valid_number("-."));
    assert!(!is_valid_number("abc"));
    assert!(!is_valid_number("1.2.3"));
}

#[test]
fn test_normalize_number() {
    assert_eq!(normalize_number("1."), "1");
    assert_eq!(normalize_number("0."), "0");
    assert_eq!(normalize_number("1.5"), "1.5");
    assert_eq!(normalize_number("."), ".");
    assert_eq!(normalize_number(""), "");
}

#[test]
fn test_clamp_number_value_within_range() {
    assert_eq!(clamp_number_value("1.5", Some(0.0), Some(2.0)), "1.5");
}

#[test]
fn test_clamp_number_value_below_min() {
    assert_eq!(clamp_number_value("-1", Some(0.0), Some(2.0)), "0");
}

#[test]
fn test_clamp_number_value_above_max() {
    assert_eq!(clamp_number_value("3", Some(0.0), Some(2.0)), "2");
}

#[test]
fn test_clamp_number_value_empty() {
    assert_eq!(clamp_number_value("", Some(0.0), Some(2.0)), "");
}

#[test]
fn test_clamp_number_value_partial_input() {
    assert_eq!(clamp_number_value("1.", Some(0.0), Some(2.0)), "1.");
}

#[test]
fn test_clamp_number_value_no_bounds() {
    assert_eq!(clamp_number_value("100", None, None), "100");
}
