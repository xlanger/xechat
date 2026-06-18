//! 日期时间格式化工具。
//!
//! 根据用户时区偏好将 UTC 时间转换为本地时间并格式化。
//! 时区偏好存储在 `AppStore.timezone` signal 中，
//! 支持系统本地时区（`"system"`）和 IANA 时区标识符。

use chrono::{DateTime, Utc, Local};
use chrono_tz::Tz;

/// 将日期相关格式别名解析为 chrono 格式字符串。
fn match_date_token(token: &str) -> Option<&'static str> {
    match token {
        "date" => Some("%Y-%m-%d"),
        "datetime" => Some("%Y-%m-%d %H:%M"),
        _ => None,
    }
}

/// 将时间相关格式别名解析为 chrono 格式字符串。
fn match_time_token(token: &str) -> Option<&'static str> {
    match token {
        "time" => Some("%H:%M"),
        "short" => Some("%m-%d %H:%M"),
        _ => None,
    }
}

/// 将格式别名解析为 chrono 格式字符串，未知别名原样返回。
fn match_format_token(token: &str) -> &str {
    match_date_token(token)
        .or_else(|| match_time_token(token))
        .unwrap_or(token)
}

/// 将格式别名解析为 chrono 格式字符串。
pub fn resolve_format_pattern(fmt: &str) -> &str {
    match_format_token(fmt)
}

/// 将 UTC 时间按用户偏好时区格式化输出。
pub fn format_with_tz(dt: &DateTime<Utc>, pattern: &str, tz_pref: &str) -> String {
    if tz_pref.is_empty() || tz_pref == "system" {
        dt.with_timezone(&Local).format(pattern).to_string()
    } else {
        match tz_pref.parse::<Tz>() {
            Ok(tz) => dt.with_timezone(&tz).format(pattern).to_string(),
            Err(_) => dt.with_timezone(&Local).format(pattern).to_string(),
        }
    }
}

/// 将 UTC 时间转换为用户偏好时区并按指定格式输出。
///
/// 格式规则：
/// - `date`：`%Y-%m-%d`
/// - `datetime`：`%Y-%m-%d %H:%M`
/// - `time`：`%H:%M`
/// - `short`：`%m-%d %H:%M`
/// - 其他值直接作为 chrono 格式字符串使用
pub fn format_datetime(dt: &DateTime<Utc>, fmt: &str, tz_pref: &str) -> String {
    let pattern = resolve_format_pattern(fmt);
    format_with_tz(dt, pattern, tz_pref)
}

/// 智能格式化消息时间：今天显示 `HH:MM`，否则显示 `YYYY-MM-DD HH:MM`。
///
/// 内部通过 `format_datetime` 比较日期字符串判断是否为今天，
/// 避免了 `DateTime<Local>` 与 IANA 时区之间的类型转换问题。
pub fn format_smart_time(dt: &DateTime<Utc>, tz_pref: &str) -> String {
    let msg_date = format_datetime(dt, "date", tz_pref);
    let today_date = format_datetime(&Utc::now(), "date", tz_pref);
    if msg_date == today_date {
        format_datetime(dt, "time", tz_pref)
    } else {
        format_datetime(dt, "datetime", tz_pref)
    }
}
