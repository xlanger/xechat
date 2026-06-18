//! SCSS detection utilities.

/// Checks for SCSS-specific indicator substrings.
///
/// Returns `true` if `content` contains any SCSS-only syntax marker such as
/// `$` variables, `@mixin`, `@include`, `@function`, `@extend`, or the
/// control-flow at-rules (`@if`, `@else`, `@for`, `@each`, `@while`).
#[inline]
fn has_scss_indicators(content: &str) -> bool {
    let scss_indicators: &[&str] = &[
        "$", // Variables
        "@mixin", "@include", "@function", "@extend",
        "@if", "@else", "@for", "@each", "@while",
    ];
    for indicator in scss_indicators {
        if content.contains(indicator) {
            return true;
        }
    }
    false
}

/// Checks if `&` appears at a position inside braces.
///
/// Returns `true` if `ch` is `&` and `in_braces > 0` (i.e. inside a declaration block),
/// `false` otherwise.
#[inline]
fn check_ampersand_at_pos(ch: char, in_braces: i32) -> bool {
    ch == '&' && in_braces > 0
}

/// Updates brace depth based on a character.
///
/// Returns the new brace depth after processing `ch`.
#[inline]
fn update_brace_depth(ch: char, in_braces: i32) -> i32 {
    match ch {
        '{' => in_braces + 1,
        '}' => in_braces - 1,
        _ => in_braces,
    }
}

/// Checks for a `&` parent selector nested inside braces.
///
/// Returns `true` if an `&` character appears while at least one `{` is
/// currently open (i.e. inside a declaration block).
#[inline]
fn has_nested_ampersand(content: &str) -> bool {
    let mut in_braces: i32 = 0;
    for ch in content.chars() {
        if check_ampersand_at_pos(ch, in_braces) {
            return true;
        }
        in_braces = update_brace_depth(ch, in_braces);
    }
    false
}

/// Checks if CSS content looks like SCSS (has SCSS-specific syntax).
#[inline]
pub fn looks_like_scss(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return false;
    }
    if has_scss_indicators(trimmed) {
        return true;
    }
    if has_nested_ampersand(trimmed) {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_string_is_not_scss() {
        assert!(!looks_like_scss(""));
        assert!(!looks_like_scss("   "));
        assert!(!looks_like_scss("\n\t"));
    }

    #[test]
    fn test_plain_css_is_not_scss() {
        assert!(!looks_like_scss(".a { color: red; }"));
        assert!(!looks_like_scss("body { margin: 0; } div { padding: 10px; }"));
        assert!(!looks_like_scss("@media (max-width: 600px) { .a { color: red; } }"));
    }

    #[test]
    fn test_scss_with_variables() {
        assert!(looks_like_scss("$color: red; .a { color: $color; }"));
    }

    #[test]
    fn test_scss_with_mixin() {
        assert!(looks_like_scss("@mixin foo { color: red; }"));
    }

    #[test]
    fn test_scss_with_include() {
        assert!(looks_like_scss("@include foo;"));
    }

    #[test]
    fn test_scss_with_function() {
        assert!(looks_like_scss("@function foo() { @return 1; }"));
    }

    #[test]
    fn test_scss_with_extend() {
        assert!(looks_like_scss(".a { color: red; } .b { @extend .a; }"));
    }

    #[test]
    fn test_scss_with_if() {
        assert!(looks_like_scss("@if true { color: red; }"));
    }

    #[test]
    fn test_scss_with_else() {
        assert!(looks_like_scss("@else { color: blue; }"));
    }

    #[test]
    fn test_scss_with_for() {
        assert!(looks_like_scss("@for $i from 1 through 3 { .a-#{$i} { } }"));
    }

    #[test]
    fn test_scss_with_each() {
        assert!(looks_like_scss("@each $i in 1, 2, 3 { }"));
    }

    #[test]
    fn test_scss_with_while() {
        assert!(looks_like_scss("@while $i > 0 { }"));
    }

    #[test]
    fn test_scss_nested_ampersand() {
        assert!(looks_like_scss(".a { &:hover { color: red; } }"));
    }

    #[test]
    fn test_nested_rules_without_ampersand_is_not_scss() {
        // Nested rules but no & and no other SCSS indicators → false
        assert!(!looks_like_scss(".a { .b { color: red; } }"));
    }

    // ---- Tests for new helper functions ----

    #[test]
    fn test_check_ampersand_at_pos_inside_braces() {
        assert!(check_ampersand_at_pos('&', 1));
        assert!(check_ampersand_at_pos('&', 2));
    }

    #[test]
    fn test_check_ampersand_at_pos_outside_braces() {
        assert!(!check_ampersand_at_pos('&', 0));
    }

    #[test]
    fn test_check_ampersand_at_pos_non_ampersand() {
        assert!(!check_ampersand_at_pos('{', 1));
        assert!(!check_ampersand_at_pos('a', 1));
    }

    #[test]
    fn test_update_brace_depth_open() {
        assert_eq!(update_brace_depth('{', 0), 1);
        assert_eq!(update_brace_depth('{', 2), 3);
    }

    #[test]
    fn test_update_brace_depth_close() {
        assert_eq!(update_brace_depth('}', 1), 0);
        assert_eq!(update_brace_depth('}', 3), 2);
    }

    #[test]
    fn test_update_brace_depth_other() {
        assert_eq!(update_brace_depth('a', 1), 1);
        assert_eq!(update_brace_depth('&', 2), 2);
    }
}
