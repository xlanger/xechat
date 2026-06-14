use xechat::utils::markdown::{
    preprocess_math, try_skip_triple_backtick, try_skip_inline_code,
    try_convert_bracket_display, try_convert_paren_inline,
    try_convert_ddollar_display, try_convert_dollar_inline,
};

// ── preprocess_math ─────────────────────────────────────────────

#[test]
fn test_preprocess_math_plain_text() {
    assert_eq!(preprocess_math("hello world"), "hello world");
}

#[test]
fn test_preprocess_math_empty() {
    assert_eq!(preprocess_math(""), "");
}

#[test]
fn test_preprocess_math_dollar_inline() {
    assert_eq!(preprocess_math("$x+y$"), "$x+y$");
}

#[test]
fn test_preprocess_math_ddollar_display() {
    assert_eq!(preprocess_math("$$x^2$$"), "$$x^2$$");
}

#[test]
fn test_preprocess_math_bracket_display() {
    assert_eq!(preprocess_math(r"\[x^2\]"), "$$x^2$$");
}

#[test]
fn test_preprocess_math_paren_inline() {
    assert_eq!(preprocess_math(r"\(x+y\)"), "$x+y$");
}

#[test]
fn test_preprocess_math_bracket_multiline() {
    let input = "\\[x^2\n+ y^2\\]";
    let result = preprocess_math(input);
    assert!(result.contains("$$"), "Should convert to $$");
    assert!(!result.contains('\n'), "Should flatten newlines");
}

#[test]
fn test_preprocess_math_paren_multiline() {
    let input = r"\(x +
\beta\)";
    let result = preprocess_math(input);
    assert!(result.starts_with('$'), "Should convert to $");
    assert!(!result.contains('\n'), "Should flatten newlines");
}

#[test]
fn test_preprocess_math_skip_code_block() {
    let input = "```rust\n$not_math$\n```";
    assert_eq!(preprocess_math(input), input);
}

#[test]
fn test_preprocess_math_skip_inline_code() {
    let input = "`$not_math$`";
    assert_eq!(preprocess_math(input), input);
}

#[test]
fn test_preprocess_math_unclosed_bracket() {
    let result = preprocess_math(r"\[unclosed");
    // When \[ is unclosed, it outputs "\[" then continues processing remaining chars
    assert!(result.starts_with("\\[") || result.starts_with("$$"), "Should handle unclosed \\[");
}

#[test]
fn test_preprocess_math_unclosed_paren() {
    let result = preprocess_math(r"\(unclosed");
    assert!(result.starts_with("\\(") || result.starts_with("$"), "Should handle unclosed \\(");
}

#[test]
fn test_preprocess_math_unclosed_ddollar() {
    let result = preprocess_math("$$unclosed");
    assert!(result.starts_with("$$"), "Should handle unclosed $$");
}

#[test]
fn test_preprocess_math_unclosed_dollar() {
    let result = preprocess_math("$unclosed");
    assert!(result.starts_with("$"), "Should handle unclosed $");
}

#[test]
fn test_preprocess_math_mixed() {
    let input = r"Inline \(x\) and display \[y\] with $z$ and $$w$$";
    let result = preprocess_math(input);
    assert!(result.contains("$x$"), "Should convert \\(...\\) to $...$");
    assert!(result.contains("$$y$$"), "Should convert \\[...\\] to $$...$$");
    assert!(result.contains("$z$"), "Should keep $...$");
    assert!(result.contains("$$w$$"), "Should keep $$...$$");
}

// ── try_skip_triple_backtick ────────────────────────────────────

#[test]
fn test_try_skip_triple_backtick_found() {
    let chars: Vec<char> = "```code```rest".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_skip_triple_backtick(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "```code```");
    assert_eq!(pos, 10);
}

#[test]
fn test_try_skip_triple_backtick_not_backtick() {
    let chars: Vec<char> = "hello".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_skip_triple_backtick(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_skip_triple_backtick_unclosed() {
    let chars: Vec<char> = "```code".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_skip_triple_backtick(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "```code");
    assert_eq!(pos, chars.len());
}

// ── try_skip_inline_code ────────────────────────────────────────

#[test]
fn test_try_skip_inline_code_found() {
    let chars: Vec<char> = "`code`rest".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_skip_inline_code(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "`code`");
    assert_eq!(pos, 6);
}

#[test]
fn test_try_skip_inline_code_not_backtick() {
    let chars: Vec<char> = "hello".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_skip_inline_code(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_skip_inline_code_triple_backtick_rejected() {
    let chars: Vec<char> = "```code```".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_skip_inline_code(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_skip_inline_code_unclosed() {
    let chars: Vec<char> = "`code".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_skip_inline_code(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "`code");
    assert_eq!(pos, chars.len());
}

// ── try_convert_bracket_display ─────────────────────────────────

#[test]
fn test_try_convert_bracket_display_found() {
    let chars: Vec<char> = r"\[x^2\]rest".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_bracket_display(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "$$x^2$$");
}

#[test]
fn test_try_convert_bracket_display_not_bracket() {
    let chars: Vec<char> = "hello".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_convert_bracket_display(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_convert_bracket_display_unclosed() {
    let chars: Vec<char> = r"\[unclosed".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_bracket_display(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "\\[");
}

// ── try_convert_paren_inline ────────────────────────────────────

#[test]
fn test_try_convert_paren_inline_found() {
    let chars: Vec<char> = r"\(x+y\)rest".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_paren_inline(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "$x+y$");
}

#[test]
fn test_try_convert_paren_inline_not_paren() {
    let chars: Vec<char> = "hello".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_convert_paren_inline(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_convert_paren_inline_unclosed() {
    let chars: Vec<char> = r"\(unclosed".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_paren_inline(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "\\(");
}

// ── try_convert_ddollar_display ─────────────────────────────────

#[test]
fn test_try_convert_ddollar_display_found() {
    let chars: Vec<char> = "$$x^2$$rest".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_ddollar_display(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "$$x^2$$");
}

#[test]
fn test_try_convert_ddollar_display_not_ddollar() {
    let chars: Vec<char> = "hello".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_convert_ddollar_display(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_convert_ddollar_display_unclosed() {
    let chars: Vec<char> = "$$unclosed".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_ddollar_display(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "$$");
}

// ── try_convert_dollar_inline ───────────────────────────────────

#[test]
fn test_try_convert_dollar_inline_found() {
    let chars: Vec<char> = "$x+y$rest".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_dollar_inline(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "$x+y$");
}

#[test]
fn test_try_convert_dollar_inline_not_dollar() {
    let chars: Vec<char> = "hello".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_convert_dollar_inline(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_convert_dollar_inline_ddollar_rejected() {
    let chars: Vec<char> = "$$x^2$$".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(!try_convert_dollar_inline(&chars, pos, &mut result, &mut pos));
}

#[test]
fn test_try_convert_dollar_inline_unclosed() {
    let chars: Vec<char> = "$unclosed".chars().collect();
    let mut result = String::new();
    let mut pos = 0;
    assert!(try_convert_dollar_inline(&chars, pos, &mut result, &mut pos));
    assert_eq!(result, "$");
}
