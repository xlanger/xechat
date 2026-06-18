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

/// Checks for a `&` parent selector nested inside braces.
///
/// Returns `true` if an `&` character appears while at least one `{` is
/// currently open (i.e. inside a declaration block).
#[inline]
fn has_nested_ampersand(content: &str) -> bool {
    let mut in_braces = 0;
    for ch in content.chars() {
        match ch {
            '{' => in_braces += 1,
            '}' => in_braces -= 1,
            '&' if in_braces > 0 => return true,
            _ => {}
        }
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
}
