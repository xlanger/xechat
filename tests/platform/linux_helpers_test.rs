use xechat::platform::SystemTheme;

/// 模拟 linux 模块的 helper 函数。
/// 由于 linux 模块仅在 target_os = "linux" 下编译，
/// 这里直接复制函数逻辑进行测试。
fn is_dark_theme_value(line: &str) -> bool {
    line.contains("=true") || line.contains("=1")
}

fn is_light_theme_value(line: &str) -> bool {
    line.contains("=false") || line.contains("=0")
}

fn parse_gtk4_theme_line(line: &str) -> Option<SystemTheme> {
    if !line.starts_with("gtk-application-prefer-dark-theme") {
        return None;
    }
    if is_dark_theme_value(line) {
        Some(SystemTheme::Dark)
    } else if is_light_theme_value(line) {
        Some(SystemTheme::Light)
    } else {
        None
    }
}

fn parse_theme_from_string(s: &str) -> Option<SystemTheme> {
    let lower = s.to_lowercase();
    if lower.contains("dark") {
        Some(SystemTheme::Dark)
    } else if lower.contains("light") {
        Some(SystemTheme::Light)
    } else {
        None
    }
}

// ── is_dark_theme_value ─────────────────────────────────────────

#[test]
fn test_is_dark_theme_value_true() {
    assert!(is_dark_theme_value("gtk-application-prefer-dark-theme=true"));
}

#[test]
fn test_is_dark_theme_value_one() {
    assert!(is_dark_theme_value("gtk-application-prefer-dark-theme=1"));
}

#[test]
fn test_is_dark_theme_value_false() {
    assert!(!is_dark_theme_value("gtk-application-prefer-dark-theme=false"));
}

#[test]
fn test_is_dark_theme_value_zero() {
    assert!(!is_dark_theme_value("gtk-application-prefer-dark-theme=0"));
}

#[test]
fn test_is_dark_theme_value_other() {
    assert!(!is_dark_theme_value("gtk-application-prefer-dark-theme=maybe"));
}

// ── is_light_theme_value ────────────────────────────────────────

#[test]
fn test_is_light_theme_value_false() {
    assert!(is_light_theme_value("gtk-application-prefer-dark-theme=false"));
}

#[test]
fn test_is_light_theme_value_zero() {
    assert!(is_light_theme_value("gtk-application-prefer-dark-theme=0"));
}

#[test]
fn test_is_light_theme_value_true() {
    assert!(!is_light_theme_value("gtk-application-prefer-dark-theme=true"));
}

#[test]
fn test_is_light_theme_value_one() {
    assert!(!is_light_theme_value("gtk-application-prefer-dark-theme=1"));
}

// ── parse_gtk4_theme_line ───────────────────────────────────────

#[test]
fn test_parse_gtk4_theme_line_dark_true() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=true"),
        Some(SystemTheme::Dark)
    );
}

#[test]
fn test_parse_gtk4_theme_line_dark_one() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=1"),
        Some(SystemTheme::Dark)
    );
}

#[test]
fn test_parse_gtk4_theme_line_light_false() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=false"),
        Some(SystemTheme::Light)
    );
}

#[test]
fn test_parse_gtk4_theme_line_light_zero() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=0"),
        Some(SystemTheme::Light)
    );
}

#[test]
fn test_parse_gtk4_theme_line_unrecognized_value() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=maybe"),
        None
    );
}

#[test]
fn test_parse_gtk4_theme_line_wrong_key() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-theme-name=Adwaita"),
        None
    );
}

#[test]
fn test_parse_gtk4_theme_line_empty() {
    assert_eq!(parse_gtk4_theme_line(""), None);
}

#[test]
fn test_parse_gtk4_theme_line_partial_prefix() {
    // "gtk-application" is not the full key prefix
    assert_eq!(parse_gtk4_theme_line("gtk-application=true"), None);
}

// ── parse_theme_from_string ─────────────────────────────────────

#[test]
fn test_parse_theme_from_string_dark() {
    assert_eq!(parse_theme_from_string("prefer-dark"), Some(SystemTheme::Dark));
}

#[test]
fn test_parse_theme_from_string_light() {
    assert_eq!(parse_theme_from_string("prefer-light"), Some(SystemTheme::Light));
}

#[test]
fn test_parse_theme_from_string_mixed_case() {
    assert_eq!(parse_theme_from_string("Prefer-DARK"), Some(SystemTheme::Dark));
    assert_eq!(parse_theme_from_string("Prefer-Light"), Some(SystemTheme::Light));
}

#[test]
fn test_parse_theme_from_string_no_match() {
    assert_eq!(parse_theme_from_string("default"), None);
}

#[test]
fn test_parse_theme_from_string_empty() {
    assert_eq!(parse_theme_from_string(""), None);
}

#[test]
fn test_parse_theme_from_string_dark_priority() {
    // "dark" should take priority over "light" if both present
    assert_eq!(parse_theme_from_string("dark-light"), Some(SystemTheme::Dark));
}
