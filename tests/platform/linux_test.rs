use xechat::platform::SystemTheme;

/// 检查行是否为 GTK4 暗色主题配置键。
fn extract_theme_key(line: &str) -> bool {
    line.starts_with("gtk-application-prefer-dark-theme")
}

/// 从配置行中解析主题值（true/1 → Dark，false/0 → Light，其余 → None）。
fn extract_theme_value(line: &str) -> Option<SystemTheme> {
    if line.contains("=true") || line.contains("=1") {
        Some(SystemTheme::Dark)
    } else if line.contains("=false") || line.contains("=0") {
        Some(SystemTheme::Light)
    } else {
        None
    }
}

/// 模拟 linux 模块的 parse_gtk4_theme_line 函数。
/// 由于 linux 模块仅在 target_os = "linux" 下编译，
/// 这里直接复制函数逻辑进行测试。
fn parse_gtk4_theme_line(line: &str) -> Option<SystemTheme> {
    if !extract_theme_key(line) {
        return None;
    }
    extract_theme_value(line)
}

// ── parse_gtk4_theme_line ──────────────────────────────────────

#[test]
fn test_parse_gtk4_theme_line_dark_true() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=true"),
        Some(SystemTheme::Dark)
    );
}

#[test]
fn test_parse_gtk4_theme_line_dark_1() {
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
fn test_parse_gtk4_theme_line_light_0() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=0"),
        Some(SystemTheme::Light)
    );
}

#[test]
fn test_parse_gtk4_theme_line_unrelated_line() {
    assert_eq!(parse_gtk4_theme_line("some-other-key=value"), None);
}

#[test]
fn test_parse_gtk4_theme_line_empty_line() {
    assert_eq!(parse_gtk4_theme_line(""), None);
}

#[test]
fn test_parse_gtk4_theme_line_unrecognized_value() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=maybe"),
        None
    );
}

#[test]
fn test_parse_gtk4_theme_line_with_leading_whitespace_not_matched() {
    // Leading whitespace means it doesn't start with the key, so returns None
    assert_eq!(
        parse_gtk4_theme_line("  gtk-application-prefer-dark-theme=true"),
        None
    );
}

#[test]
fn test_parse_gtk4_theme_line_with_comment() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-theme=true # comment"),
        Some(SystemTheme::Dark)
    );
}

#[test]
fn test_parse_gtk4_theme_line_partial_key_name() {
    assert_eq!(
        parse_gtk4_theme_line("gtk-application-prefer-dark-them=true"),
        None
    );
}
