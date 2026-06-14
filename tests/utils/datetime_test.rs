use chrono::{TimeZone, Utc};
use xechat::utils::datetime::{resolve_format_pattern, format_with_tz};

// ── resolve_format_pattern ──────────────────────────────────────

#[test]
fn test_resolve_format_pattern_date() {
    assert_eq!(resolve_format_pattern("date"), "%Y-%m-%d");
}

#[test]
fn test_resolve_format_pattern_datetime() {
    assert_eq!(resolve_format_pattern("datetime"), "%Y-%m-%d %H:%M");
}

#[test]
fn test_resolve_format_pattern_time() {
    assert_eq!(resolve_format_pattern("time"), "%H:%M");
}

#[test]
fn test_resolve_format_pattern_short() {
    assert_eq!(resolve_format_pattern("short"), "%m-%d %H:%M");
}

#[test]
fn test_resolve_format_pattern_custom() {
    assert_eq!(resolve_format_pattern("%Y/%m/%d"), "%Y/%m/%d");
}

#[test]
fn test_resolve_format_pattern_empty() {
    assert_eq!(resolve_format_pattern(""), "");
}

// ── format_with_tz ──────────────────────────────────────────────

#[test]
fn test_format_with_tz_system() {
    let dt = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
    let result = format_with_tz(&dt, "%Y-%m-%d %H:%M", "system");
    // Result depends on local timezone, just verify it's not empty
    assert!(!result.is_empty());
}

#[test]
fn test_format_with_tz_empty_tz() {
    let dt = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
    let result = format_with_tz(&dt, "%Y-%m-%d %H:%M", "");
    // Empty tz should fall back to system
    assert!(!result.is_empty());
}

#[test]
fn test_format_with_tz_iana_timezone() {
    let dt = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
    let result = format_with_tz(&dt, "%Y-%m-%d %H:%M", "Asia/Shanghai");
    assert_eq!(result, "2025-01-15 18:30");
}

#[test]
fn test_format_with_tz_utc_timezone() {
    let dt = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
    let result = format_with_tz(&dt, "%Y-%m-%d %H:%M", "UTC");
    assert_eq!(result, "2025-01-15 10:30");
}

#[test]
fn test_format_with_tz_invalid_timezone_falls_back() {
    let dt = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
    let result = format_with_tz(&dt, "%Y-%m-%d %H:%M", "Invalid/Timezone");
    // Should fall back to local timezone, not panic
    assert!(!result.is_empty());
}

#[test]
fn test_format_with_tz_date_only() {
    let dt = Utc.with_ymd_and_hms(2025, 6, 15, 0, 0, 0).unwrap();
    let result = format_with_tz(&dt, "%Y-%m-%d", "UTC");
    assert_eq!(result, "2025-06-15");
}

#[test]
fn test_format_with_tz_time_only() {
    let dt = Utc.with_ymd_and_hms(2025, 1, 15, 14, 45, 0).unwrap();
    let result = format_with_tz(&dt, "%H:%M", "UTC");
    assert_eq!(result, "14:45");
}
