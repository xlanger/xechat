use xechat::utils::markdown::{render_to_html, SyntaxTheme};

fn render_dark(content: &str) -> String {
    render_to_html(content, SyntaxTheme::Dark)
}

#[test]
fn test_render_plain_text() {
    let html = render_dark("hello world");
    assert!(html.contains("hello world"));
    assert!(html.contains("<p"));
}

#[test]
fn test_render_heading_h1() {
    let html = render_dark("# Title");
    assert!(html.contains("Title"));
    assert!(html.contains("<h1>"));
}

#[test]
fn test_render_heading_h2() {
    let html = render_dark("## Subtitle");
    assert!(html.contains("Subtitle"));
    assert!(html.contains("<h2>"));
}

#[test]
fn test_render_heading_h3() {
    let html = render_dark("### Section");
    assert!(html.contains("Section"));
    assert!(html.contains("<h3>"));
}

#[test]
fn test_render_bold() {
    let html = render_dark("**bold**");
    assert!(html.contains("<strong"));
    assert!(html.contains("bold"));
    assert!(html.contains("</strong>"));
}

#[test]
fn test_render_italic() {
    let html = render_dark("*italic*");
    assert!(html.contains("<em"));
    assert!(html.contains("italic"));
    assert!(html.contains("</em>"));
}

#[test]
fn test_render_inline_code() {
    let html = render_dark("`code`");
    assert!(html.contains("<code"));
    assert!(html.contains("code"));
    assert!(html.contains("</code>"));
}

#[test]
fn test_render_code_block_with_lang() {
    let html = render_dark("```rust\nfn main() {}\n```");
    assert!(html.contains("rust"));
    assert!(html.contains("fn"));
    assert!(html.contains("main"));
    assert!(html.contains("<pre"));
}

#[test]
fn test_render_code_block_without_lang() {
    let html = render_dark("```\nsome code\n```");
    assert!(html.contains("some code"));
    assert!(html.contains("<pre"));
}

#[test]
fn test_render_link() {
    let html = render_dark("[click](https://example.com)");
    assert!(html.contains("<a href=\"https://example.com\""));
    assert!(html.contains("click"));
    assert!(html.contains("</a>"));
}

#[test]
fn test_render_unordered_list() {
    let html = render_dark("- item1\n- item2");
    assert!(html.contains("<ul"));
    assert!(html.contains("<li"));
    assert!(html.contains("item1"));
    assert!(html.contains("item2"));
    assert!(html.contains("</ul>"));
}

#[test]
fn test_render_ordered_list() {
    let html = render_dark("1. first\n2. second");
    assert!(html.contains("<ol"));
    assert!(html.contains("<li"));
    assert!(html.contains("first"));
    assert!(html.contains("</ol>"));
}

#[test]
fn test_render_blockquote() {
    let html = render_dark("> quote text");
    assert!(html.contains("<blockquote"));
    assert!(html.contains("quote text"));
    assert!(html.contains("</blockquote>"));
}

#[test]
fn test_render_horizontal_rule() {
    let html = render_dark("---");
    assert!(html.contains("<hr"));
}

#[test]
fn test_render_strikethrough() {
    let html = render_dark("~~deleted~~");
    assert!(html.contains("<del"));
    assert!(html.contains("deleted"));
    assert!(html.contains("</del>"));
}

#[test]
fn test_render_empty() {
    let html = render_dark("");
    assert!(html.is_empty());
}

#[test]
fn test_render_escapes_text_content() {
    let html = render_dark("1 < 2 & 3 > 0");
    assert!(html.contains("&lt;"));
    assert!(html.contains("&amp;"));
    assert!(html.contains("&gt;"));
}

#[test]
fn test_render_image() {
    let html = render_dark("![alt](https://example.com/img.png)");
    assert!(html.contains("<img src=\"https://example.com/img.png\""));
}

#[test]
fn test_render_code_block_escapes_html() {
    let html = render_dark("```\n<div>test</div>\n```");
    assert!(html.contains("&lt;div&gt;"));
}

#[test]
fn test_render_code_highlight_rust() {
    let html = render_dark("```rust\nfn main() {}\n```");
    assert!(html.contains("style=") || html.contains("class="));
}

#[test]
fn test_render_inline_math() {
    let html = render_dark("$E=mc^2$");
    assert!(html.contains("katex"), "Expected katex class in output");
}

#[test]
fn test_render_display_math() {
    let html = render_dark("$$x^2 + y^2 = z^2$$");
    assert!(html.contains("katex"), "Expected katex class in output");
}

#[test]
fn test_render_backslash_math() {
    let html = render_dark(r"$\partial_\lambda \phi$");
    assert!(html.contains("katex"), "Expected katex class in output");
}

#[test]
fn test_render_math_code_block() {
    let html = render_dark("```math\nx^2\n```");
    assert!(html.contains("katex") || html.contains("x^2") || html.contains("math"));
}

#[test]
fn test_render_invalid_math_fallback() {
    let html = render_dark("$\\invalid{cmd$");
    assert!(!html.is_empty());
}

#[test]
fn test_render_mermaid_code_block() {
    let html = render_dark("```mermaid\nflowchart LR; A-->B\n```");
    assert!(html.contains("mermaid") || html.contains("flowchart"));
}

#[test]
fn test_render_mermaid_invalid_fallback() {
    let html = render_dark("```mermaid\ninvalid syntax here @@@\n```");
    eprintln!("=== DIAGNOSTIC: mermaid invalid fallback ===");
    eprintln!("Output:\n{}", html);
    assert!(!html.is_empty());
}

#[test]
fn test_diagnostic_code_block_output() {
    let html = render_dark("```rust\nfn main() {}\n```");
    eprintln!("=== DIAGNOSTIC: rust code block ===");
    eprintln!("Output:\n{}", html);
    assert!(html.contains("<pre"));
    assert!(html.contains(">rust</span>"), "Language label 'rust' should appear in header span");
}

#[test]
fn test_diagnostic_plain_code_block_output() {
    let html = render_dark("```\nplain code\n```");
    eprintln!("=== DIAGNOSTIC: plain code block ===");
    eprintln!("Output:\n{}", html);
    assert!(html.contains("<pre"));
}

#[test]
fn test_diagnostic_real_ai_output() {
    let input = r"The gradient $\nabla f(x)$ is computed as:

$$\nabla f(x) = \left[\frac{\partial f}{\partial x_1}\right]$$

Use `grad_f()` to compute it.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: Real AI output ===");
    eprintln!("Input: {}", input);
    eprintln!("Output: {}", html);
    assert!(html.contains("katex"), "Expected katex in output");
    assert!(html.contains("<code"), "Expected <code> in output");
}

#[test]
fn test_diagnostic_ricci_tensor() {
    let input = r"好的，给你一个相对复杂的数学公式——来自广义相对论的里奇曲率张量的定义：

$$R_{\mu\nu} = \partial_\lambda \Gamma^\lambda_{\mu\nu} - \partial_\nu \Gamma^\lambda_{\mu\lambda} + \Gamma^\lambda_{\lambda\rho} \Gamma^\rho_{\mu\nu} - \Gamma^\lambda_{\nu\rho} \Gamma^\rho_{\mu\lambda}$$

其中：

$\partial_{\lambda}$ 表示对坐标 $x^\lambda$ 的偏导数。
$\Gamma^{\lambda}_{\mu\nu}$ 是克里斯托费尔符号（联络系数），由度规张量及其对坐标的导数构成。
重复指标（上下成对）表示爱因斯坦求和约定。";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: Ricci Tensor ===");
    eprintln!("Input: {}", input);
    eprintln!("Output: {}", html);
    assert!(html.contains("katex"), "Expected katex in output, got: {}", &html[..html.len().min(500)]);
}

#[test]
fn test_diagnostic_comrak_raw_output() {
    use comrak::markdown_to_html_with_plugins;
    use comrak::options::Plugins;

    let input = r"$\partial_{\lambda}$ 表示对坐标 $x^\lambda$ 的偏导数。";

    let mut options = comrak::Options::default();
    options.extension.math_dollars = true;
    options.render.r#unsafe = true;
    let plugins = Plugins::default();

    let raw = markdown_to_html_with_plugins(input, &options, &plugins);
    eprintln!("=== DIAGNOSTIC: comrak raw output ===");
    eprintln!("Input: {}", input);
    eprintln!("Raw HTML: {}", raw);

    let has_math_span = raw.contains("data-math-style");
    eprintln!("Has data-math-style: {}", has_math_span);
}

#[test]
fn test_comrak_math_dollar_parsing_directly() {
    use comrak::markdown_to_html_with_plugins;
    use comrak::options::Plugins;

    let input = "- $\\partial_{\\lambda}$ test\n- $$display$$ test";
    let mut options = comrak::Options::default();
    options.extension.math_dollars = true;
    options.render.r#unsafe = true;
    let plugins = Plugins::default();
    let raw_html = markdown_to_html_with_plugins(input, &options, &plugins);

    eprintln!("=== DIAGNOSTIC: comrak direct ===");
    eprintln!("Input: [{}]", input);
    eprintln!("Raw HTML: [{}]", raw_html);
    eprintln!("Has data-math-style: {}", raw_html.contains("data-math-style"));
    eprintln!("Has literal $: {}", raw_html.contains("$"));
    assert!(raw_html.contains("data-math-style"), "comrak should generate math spans for $...$");
}

#[test]
fn test_multiline_display_math() {
    let input = r"The Christoffel symbols are defined as:

$$\Gamma^{\lambda}_{\mu\nu} = \frac{1}{2} g^{\lambda\rho}
\left( \partial_\mu g_{\nu\rho} + \partial_\nu g_{\mu\rho} - \partial_\rho g_{\mu\nu} \right)$$

This is the standard definition.";

    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: multiline display math ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);

    let has_display_katex = html.contains("katex-display");
    let has_raw_frac = html.contains(r"\frac") && {
        let frac_pos = html.find(r"\frac").unwrap_or(999999);
        frac_pos < html.len() && !html[..frac_pos].ends_with("katex-html\">")
    };
    let has_raw_gamma = html.contains(r"\Gamma") && {
        let gamma_pos = html.find(r"\Gamma").unwrap_or(999999);
        gamma_pos < html.len() && !html[..gamma_pos].ends_with("katex-html\">")
    };

    eprintln!("Has display katex: {}, Has raw \\frac: {}, Has raw \\Gamma: {}", has_display_katex, has_raw_frac, has_raw_gamma);

    assert!(has_display_katex, "Expected display katex for multiline $$ formula");
    assert!(!has_raw_frac, "Should not have raw \\frac in output");
    assert!(!has_raw_gamma, "Should not have raw \\Gamma in output");
}

#[test]
fn test_inline_math_in_list_items() {
    let input = r"其中：

- $\partial_{\lambda}$ 表示对坐标 $x^\lambda$ 的偏导数。
- $\Gamma^{\lambda}_{\mu\nu}$ 是克里斯托费尔符号。
- 重复指标（上下成对）表示爱因斯坦求和约定。";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: inline math in list items ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(1500)]);

    let katex_count = html.matches("katex").count();
    eprintln!("Katex count: {}", katex_count);
    assert!(katex_count >= 4, "Expected at least 4 katex renders for inline math in list, got {}", katex_count);
}

#[test]
fn test_display_math_with_plus_sign() {
    let input = r"$$R_{\mu\nu} = \partial_\lambda \Gamma^\lambda_{\mu\nu} - \partial_\nu \Gamma^\lambda_{\mu\lambda} + \Gamma^\lambda_{\lambda\rho} \Gamma^\rho_{\mu\nu} - \Gamma^\lambda_{\nu\rho} \Gamma^\rho_{\mu\lambda}$$";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: display math with plus sign ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);

    let has_display_katex = html.contains("katex-display");
    let has_raw_gamma = html.contains(r"\Gamma") && {
        let gamma_pos = html.find(r"\Gamma").unwrap_or(999999);
        gamma_pos < html.len() && !html[..gamma_pos].ends_with("katex-html\">")
    };

    eprintln!("Has display katex: {}, Has raw \\Gamma: {}", has_display_katex, has_raw_gamma);
    assert!(has_display_katex, "Expected display katex for $$ formula with + sign");
    assert!(!has_raw_gamma, "Should not have raw \\Gamma in output");
}

#[test]
fn test_mixed_inline_and_display_math() {
    let input = r"The gradient $\nabla f(x)$ and the formula:

$$\nabla f(x) = \left[\frac{\partial f}{\partial x_1}\right]$$

where $\partial$ is the partial derivative.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: mixed inline and display math ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);

    let katex_count = html.matches("katex").count();
    let has_display = html.contains("katex-display");
    eprintln!("Katex count: {}, Has display: {}", katex_count, has_display);
    assert!(katex_count >= 3, "Expected at least 3 katex renders, got {}", katex_count);
    assert!(has_display, "Expected display katex for $$ block");
}

#[test]
fn test_chinese_text_with_math() {
    let input = r"好的，给你一个相对复杂的数学公式——来自广义相对论的里奇曲率张量的定义：

$$R_{\mu\nu} = \partial_\lambda \Gamma^\lambda_{\mu\nu}$$

其中：$\partial_{\lambda}$ 表示偏导数。";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: Chinese text with math ===");
    let contains_chinese = html.contains("好的") || html.contains("里奇曲率") || html.contains("偏导数");
    let katex_count = html.matches("katex").count();
    eprintln!("Contains Chinese: {}, Katex count: {}", contains_chinese, katex_count);
    assert!(contains_chinese, "Chinese text must be preserved");
    assert!(katex_count >= 2, "Expected at least 2 katex renders, got {}", katex_count);
}

#[test]
fn test_display_math_with_inline_math_in_same_paragraph() {
    let input = r"The value $x^2$ satisfies: $$x^2 + y^2 = z^2$$ where $z$ is the hypotenuse.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: display + inline in same paragraph ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);

    let katex_count = html.matches("katex").count();
    eprintln!("Katex count: {}", katex_count);
    assert!(katex_count >= 3, "Expected at least 3 katex renders, got {}", katex_count);
}

#[test]
fn test_preprocess_paren_inline_math() {
    let input = r"The gradient \(\nabla f(x)\) is computed.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: \\(...\\) inline math ===");
    eprintln!("Input: {}", input);
    eprintln!("Output: {}", &html[..html.len().min(500)]);
    assert!(html.contains("katex"), "Expected katex for \\(...\\) inline math");
}

#[test]
fn test_preprocess_bracket_display_math() {
    let input = r"The formula is:

\[\Gamma^{\lambda}_{\mu\nu} = \frac{1}{2} g^{\lambda\rho}\]

End.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: \\[...\\] display math ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(1000)]);
    assert!(html.contains("katex-display"), "Expected katex-display for \\[...\\] display math");
}

#[test]
fn test_preprocess_bracket_multiline_flatten() {
    let input = r"\[\Gamma^{\lambda}_{\mu\nu} = \frac{1}{2} g^{\lambda\rho}
\left( \partial_\mu g_{\nu\rho} + \partial_\nu g_{\mu\rho} - \partial_\rho g_{\mu\nu} \right)\]";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: \\[...\\] multiline flatten ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);
    assert!(html.contains("katex-display"), "Expected katex-display for multiline \\[...\\]");
    let has_raw_frac = html.contains(r"\frac") && {
        let pos = html.find(r"\frac").unwrap_or(999999);
        pos < html.len() && !html[..pos].ends_with("katex-html\">")
    };
    assert!(!has_raw_frac, "Should not have raw \\frac outside katex");
}

#[test]
fn test_preprocess_ddollar_multiline_flatten() {
    let input = r"$$\Gamma^{\lambda}_{\mu\nu} = \frac{1}{2} g^{\lambda\rho}
\left( \partial_\mu g_{\nu\rho} + \partial_\nu g_{\mu\rho} - \partial_\rho g_{\mu\nu} \right)$$";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: $$...$$ multiline flatten ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);
    assert!(html.contains("katex-display"), "Expected katex-display for multiline $$...$$");
    let has_raw_frac = html.contains(r"\frac") && {
        let pos = html.find(r"\frac").unwrap_or(999999);
        pos < html.len() && !html[..pos].ends_with("katex-html\">")
    };
    assert!(!has_raw_frac, "Should not have raw \\frac outside katex");
}

#[test]
fn test_preprocess_mixed_delimiters() {
    let input = r"Inline \(\Gamma^{\lambda}_{\mu\nu}\) and display:

\[\frac{\partial f}{\partial x} = \nabla f\]

Also $E=mc^2$ and $$x^2 + y^2 = z^2$$.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: mixed delimiters ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);
    let katex_count = html.matches("katex").count();
    eprintln!("Katex count: {}", katex_count);
    assert!(katex_count >= 4, "Expected at least 4 katex renders for mixed delimiters, got {}", katex_count);
}

#[test]
fn test_preprocess_unclosed_bracket() {
    let input = r"Unclosed \[\frac{1}{2} and more text.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: unclosed \\[ ===");
    eprintln!("Input: {}", input);
    eprintln!("Output: {}", &html[..html.len().min(500)]);
    assert!(!html.is_empty(), "Should not crash on unclosed \\[");
}

#[test]
fn test_preprocess_unclosed_paren() {
    let input = r"Unclosed \(\alpha and more text.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: unclosed \\( ===");
    eprintln!("Input: {}", input);
    eprintln!("Output: {}", &html[..html.len().min(500)]);
    assert!(!html.is_empty(), "Should not crash on unclosed \\(");
}

#[test]
fn test_preprocess_long_formula_with_plus_at_line_start() {
    let input = r"$$\Gamma^{\lambda}_{\mu\nu} = \frac{\partial x'^\lambda}{\partial x^\rho}
+ \frac{\partial x^\sigma}{\partial x'^\mu}
+ \frac{\partial^2 x^\rho}{\partial x'^\mu \partial x'^\nu}$$";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: long formula with + at line start ===");
    eprintln!("Input:\n{}", input);
    eprintln!("Output:\n{}", &html[..html.len().min(2000)]);
    assert!(html.contains("katex-display"), "Expected katex-display for long formula");
    let has_raw_frac = html.contains(r"\frac") && {
        let pos = html.find(r"\frac").unwrap_or(999999);
        pos < html.len() && !html[..pos].ends_with("katex-html\">")
    };
    assert!(!has_raw_frac, "Should not have raw \\frac outside katex");
}

#[test]
fn test_preprocess_paren_inline_multiline() {
    let input = r"The value \(\alpha +
\beta\) is computed.";
    let html = render_dark(input);
    eprintln!("=== DIAGNOSTIC: \\(...\\) inline multiline ===");
    eprintln!("Input: {}", input);
    eprintln!("Output: {}", &html[..html.len().min(500)]);
    assert!(html.contains("katex"), "Expected katex for multiline \\(...\\)");
}

use xechat::utils::markdown::{scan_inline_closing, is_renderable_latex, scan_display_closing, has_latex_features};

// ── scan_inline_closing ─────────────────────────────────────────

#[test]
fn test_scan_inline_closing_simple() {
    let chars: Vec<char> = "$x+y$".chars().collect();
    let (pos, found) = scan_inline_closing(&chars, 0);
    assert!(found);
    assert_eq!(pos, 4, "Closing $ should be at index 4");
}

#[test]
fn test_scan_inline_closing_no_close() {
    let chars: Vec<char> = "$x+y".chars().collect();
    let (pos, found) = scan_inline_closing(&chars, 0);
    assert!(!found);
    assert_eq!(pos, 4, "Should scan to end");
}

#[test]
fn test_scan_inline_closing_with_braces() {
    let chars: Vec<char> = "$\\frac{1}{2}$".chars().collect();
    let (_pos, found) = scan_inline_closing(&chars, 0);
    assert!(found, "Should find closing $ after balanced braces");
}

#[test]
fn test_scan_inline_closing_escaped_paren_stops() {
    let chars: Vec<char> = "$x\\)".chars().collect();
    let (_pos, found) = scan_inline_closing(&chars, 0);
    assert!(!found, "Should stop at escaped \\)");
}

#[test]
fn test_scan_inline_closing_escaped_bracket_stops() {
    let chars: Vec<char> = "$x\\]".chars().collect();
    let (_pos, found) = scan_inline_closing(&chars, 0);
    assert!(!found, "Should stop at escaped \\]");
}

#[test]
fn test_scan_inline_closing_dollar_inside_braces() {
    let chars: Vec<char> = "$\\$5$".chars().collect();
    let (_pos, found) = scan_inline_closing(&chars, 0);
    // $ inside braces should be skipped
    assert!(found, "Should find closing $ after braces");
}

#[test]
fn test_scan_inline_closing_empty_content() {
    let chars: Vec<char> = "$$".chars().collect();
    let (pos, found) = scan_inline_closing(&chars, 0);
    assert!(found, "Empty inline math should close immediately");
    assert_eq!(pos, 1);
}

// ── is_renderable_latex ─────────────────────────────────────────

#[test]
fn test_is_renderable_latex_normal() {
    assert!(is_renderable_latex("x^2 + y^2"));
}

#[test]
fn test_is_renderable_latex_empty() {
    assert!(!is_renderable_latex(""));
}

#[test]
fn test_is_renderable_latex_whitespace_only() {
    assert!(!is_renderable_latex("   "));
}

#[test]
fn test_is_renderable_latex_contains_html_tag() {
    assert!(!is_renderable_latex("x <b>bold</b>"));
}

#[test]
fn test_is_renderable_latex_contains_angle_brackets() {
    assert!(!is_renderable_latex("a < b"));
    assert!(!is_renderable_latex("a > b"));
}

#[test]
fn test_is_renderable_latex_trimmed() {
    assert!(is_renderable_latex("  x^2  "), "Should trim before checking");
}

// ── scan_display_closing ────────────────────────────────────────

#[test]
fn test_scan_display_closing_found() {
    let chars: Vec<char> = "$$x^2$$".chars().collect();
    let (pos, found) = scan_display_closing(&chars, 0);
    assert!(found, "Should find closing $$");
    assert_eq!(pos, 5, "Closing $$ should start at index 5");
}

#[test]
fn test_scan_display_closing_not_found() {
    let chars: Vec<char> = "$$x^2".chars().collect();
    let (_pos, found) = scan_display_closing(&chars, 0);
    assert!(!found, "Should not find closing $$");
}

#[test]
fn test_scan_display_closing_multiline() {
    let chars: Vec<char> = "$$x^2\n+ y^2$$".chars().collect();
    let (_pos, found) = scan_display_closing(&chars, 0);
    assert!(found, "Should find closing $$ across lines");
}

#[test]
fn test_scan_display_closing_too_short() {
    let chars: Vec<char> = "$$".chars().collect();
    let (_pos, found) = scan_display_closing(&chars, 0);
    assert!(!found, "Too short to have closing $$");
}

// ── has_latex_features ──────────────────────────────────────────

#[test]
fn test_has_latex_features_backslash() {
    assert!(has_latex_features("\\frac{1}{2}"));
}

#[test]
fn test_has_latex_features_underscore() {
    assert!(has_latex_features("x_1"));
}

#[test]
fn test_has_latex_features_caret() {
    assert!(has_latex_features("x^2"));
}

#[test]
fn test_has_latex_features_none() {
    assert!(!has_latex_features("plain text"));
}

#[test]
fn test_has_latex_features_empty() {
    assert!(!has_latex_features(""));
}
