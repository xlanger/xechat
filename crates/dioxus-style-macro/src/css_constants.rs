//! SCSS detection utilities.

/// Checks if CSS content looks like SCSS (has SCSS-specific syntax).
#[inline]
pub fn looks_like_scss(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Quick checks for SCSS-specific syntax
    let scss_indicators: &[&str] = &[
        "$",   // Variables
        "@mixin", "@include", "@function", "@extend",
        "@if", "@else", "@for", "@each", "@while",
    ];

    for indicator in scss_indicators {
        if trimmed.contains(indicator) {
            return true;
        }
    }

    // Check for nested rules (simplified: look for { inside declarations)
    let mut in_braces = 0;
    let mut chars = trimmed.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' => in_braces += 1,
            '}' => in_braces -= 1,
            _ => {}
        }
        // If we see a & after some content inside braces, it's SCSS
        if ch == '&' && in_braces > 0 {
            return true;
        }
    }

    false
}
