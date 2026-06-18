//! CSS parsing and scoping utilities.

pub fn parse_and_scope(css: &str, scope: &str, minify: bool) -> String {
    let mut scoped_css = String::with_capacity(css.len() + scope.len() * 10);
    let rules = parse_css_rules(css);
    for rule in rules {
        if let Some(scoped_rule) = scope_rule(&rule, scope) {
            scoped_css.push_str(&scoped_rule);
            if !minify {
                scoped_css.push('\n');
            }
        }
    }
    if minify {
        scoped_css = minify_css(&scoped_css);
    }
    scoped_css
}

#[inline]
fn skip_css_comment(chars: &mut std::iter::Peekable<std::str::Chars>) {
    chars.next(); // consume '*'
    while let Some(ch) = chars.next() {
        if ch == '*' && chars.peek() == Some(&'/') {
            chars.next();
            break;
        }
    }
}

#[inline]
fn handle_comment(ch: char, chars: &mut std::iter::Peekable<std::str::Chars>) -> bool {
    if ch == '/' && chars.peek() == Some(&'*') {
        skip_css_comment(chars);
        return true;
    }
    false
}

#[inline]
fn finalize_rule(current_rule: &mut String, rules: &mut Vec<String>) {
    let trimmed = current_rule.trim();
    if !trimmed.is_empty() {
        rules.push(trimmed.to_string());
    }
    current_rule.clear();
}

#[inline]
fn process_rule_char(
    ch: char,
    brace_count: usize,
    current_rule: &mut String,
    rules: &mut Vec<String>,
) -> usize {
    current_rule.push(ch);
    if ch == '{' {
        return brace_count + 1;
    }
    if ch == '}' {
        let new_count = brace_count - 1;
        if new_count == 0 {
            finalize_rule(current_rule, rules);
        }
        return new_count;
    }
    brace_count
}

#[inline]
fn parse_css_rules(css: &str) -> Vec<String> {
    let mut rules = Vec::with_capacity(16);
    let mut current_rule = String::with_capacity(128);
    let mut brace_count = 0;
    let mut chars = css.chars().peekable();

    while let Some(ch) = chars.next() {
        if handle_comment(ch, &mut chars) {
            continue;
        }
        brace_count = process_rule_char(ch, brace_count, &mut current_rule, &mut rules);
    }
    rules
}

#[inline]
fn scope_rule(rule: &str, scope: &str) -> Option<String> {
    let trimmed = rule.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('@') {
        return handle_at_rule(trimmed, scope);
    }
    let brace_pos = trimmed.find('{')?;
    let selector = trimmed[..brace_pos].trim();
    let rest = &trimmed[brace_pos + 1..];
    let declarations = if let Some(pos) = rest.rfind('}') {
        rest[..pos].trim()
    } else {
        rest.trim()
    };
    let scoped_selector = scope_selector(selector, scope);
    Some(format!("{} {{ {} }}", scoped_selector, declarations))
}

#[inline]
fn is_passthrough_at_rule(at_name: &str) -> bool {
    matches!(
        at_name,
        "keyframes" | "-webkit-keyframes" | "-moz-keyframes" | "font-face"
    )
}

#[inline]
fn is_scoping_at_rule(at_name: &str) -> bool {
    matches!(at_name, "media" | "supports" | "layer" | "container")
}

#[inline]
fn scope_nested_at_rule(rule: &str, brace_pos: usize, scope: &str) -> Option<String> {
    let condition = &rule[..brace_pos + 1];
    let rest = &rule[brace_pos + 1..];
    let close_pos = rest.rfind('}')?;
    let inner_css = &rest[..close_pos];
    let inner_rules = parse_css_rules(inner_css);
    let mut scoped_inner = String::with_capacity(inner_css.len());
    for inner_rule in inner_rules {
        if let Some(scoped) = scope_rule(&inner_rule, scope) {
            scoped_inner.push_str(&scoped);
            scoped_inner.push(' ');
        }
    }
    Some(format!("{} {} }}", condition, scoped_inner.trim()))
}

#[inline]
fn handle_at_rule(rule: &str, scope: &str) -> Option<String> {
    let at_end = rule[1..].find(|c: char| c.is_whitespace() || c == '{')
        .map(|i| i + 1)
        .unwrap_or(rule.len());
    let at_name = &rule[1..at_end].to_lowercase();

    if is_passthrough_at_rule(at_name) {
        return Some(rule.to_string());
    }
    if is_scoping_at_rule(at_name) {
        if let Some(brace_pos) = rule.find('{') {
            if let Some(scoped) = scope_nested_at_rule(rule, brace_pos, scope) {
                return Some(scoped);
            }
        }
    }
    Some(rule.to_string())
}

#[inline]
fn scope_selector(selector: &str, scope: &str) -> String {
    if !selector.contains(',') {
        return scope_single_selector(selector, scope);
    }
    let selectors: Vec<_> = selector
        .split(',')
        .map(|s| scope_single_selector(s.trim(), scope))
        .collect();
    selectors.join(", ")
}

#[inline]
fn is_ident_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-' || ch == '_'
}

#[inline]
fn consume_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut name = String::with_capacity(16);
    while let Some(&ch) = chars.peek() {
        if !is_ident_char(ch) {
            break;
        }
        name.push(ch);
        chars.next();
    }
    name
}

#[inline]
fn scope_class_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    scope: &str,
) {
    let class_name = consume_identifier(chars);
    if class_name.is_empty() {
        result.push('.');
        return;
    }
    let scoped_name = class_name.replace('_', "-");
    result.push('.');
    result.push_str(scope);
    result.push('_');
    result.push_str(&scoped_name);
}

#[inline]
fn scope_id_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
) {
    let id_name = consume_identifier(chars);
    if id_name.is_empty() {
        result.push('#');
        return;
    }
    result.push('#');
    result.push_str(&id_name);
}

#[inline]
fn scope_combinator(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
) -> bool {
    result.push(ch);
    while let Some(&next_ch) = chars.peek() {
        if next_ch == ' ' {
            chars.next();
        } else {
            break;
        }
    }
    true // at_start = true
}

#[inline]
fn is_pseudo_name_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '-'
}

#[inline]
fn extract_pseudo_name(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
) -> String {
    if chars.peek() == Some(&':') {
        result.push(chars.next().unwrap());
    }
    let mut name = String::new();
    while let Some(&ch) = chars.peek() {
        if !is_pseudo_name_char(ch) {
            break;
        }
        name.push(ch);
        result.push(ch);
        chars.next();
    }
    name
}

#[inline]
fn is_functional_pseudo(name: &str) -> bool {
    ["not", "has", "is", "where", "matches"]
        .iter()
        .any(|p| name.eq_ignore_ascii_case(p))
}

#[inline]
fn push_paren_char(content: &mut String, c: char, depth: &mut usize) -> bool {
    if c == '(' {
        *depth += 1;
        content.push(c);
        return false;
    }
    if c == ')' {
        *depth -= 1;
        if *depth == 0 {
            return true;
        }
        content.push(c);
        return false;
    }
    content.push(c);
    false
}

#[inline]
fn extract_paren_content(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut content = String::new();
    let mut depth = 1;
    while let Some(c) = chars.next() {
        if push_paren_char(&mut content, c, &mut depth) {
            break;
        }
    }
    content
}

#[inline]
fn handle_functional_pseudo(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    scope: &str,
) {
    if chars.peek() != Some(&'(') {
        return;
    }
    result.push(chars.next().unwrap());
    let inner = extract_paren_content(chars);
    let scoped = scope_selector(&inner, scope);
    result.push_str(&scoped);
    result.push(')');
}

#[inline]
fn scope_pseudo_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    scope: &str,
) {
    result.push(':');
    let name = extract_pseudo_name(chars, result);
    if is_functional_pseudo(&name) {
        handle_functional_pseudo(chars, result, scope);
    }
}

#[inline]
fn toggle_quote(in_quote: Option<char>, ch: char) -> Option<char> {
    match in_quote {
        Some(q) if q == ch => None,
        None => Some(ch),
        _ => in_quote,
    }
}

#[inline]
fn handle_escape(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    in_quote: Option<char>,
) -> Option<char> {
    if in_quote.is_some() {
        if let Some(escaped) = chars.next() {
            result.push(escaped);
        }
    }
    in_quote
}

#[inline]
fn handle_attribute_char(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    in_quote: Option<char>,
) -> Option<char> {
    result.push(ch);
    match ch {
        '"' | '\'' => toggle_quote(in_quote, ch),
        '\\' => handle_escape(chars, result, in_quote),
        _ => in_quote,
    }
}

#[inline]
fn scope_attribute_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
) {
    result.push('[');
    let mut in_quote: Option<char> = None;
    while let Some(ch) = chars.next() {
        in_quote = handle_attribute_char(ch, chars, result, in_quote);
        if in_quote.is_none() && ch == ']' {
            break;
        }
    }
}

#[inline]
fn scope_element_component(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    scope: &str,
) {
    let mut element_name = String::from(ch);
    element_name.push_str(&consume_identifier(chars));
    if element_name == "root" || element_name == "host" {
        result.push_str(&element_name);
    } else {
        result.push_str(&element_name);
        result.push_str("[data-scope=\"");
        result.push_str(scope);
        result.push_str("\"]");
    }
}

#[inline]
fn dispatch_selector_char(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    scope: &str,
    at_start: bool,
) -> bool {
    match ch {
        '.' => {
            scope_class_component(chars, result, scope);
            false
        }
        '#' => {
            scope_id_component(chars, result);
            false
        }
        ' ' | '>' | '+' | '~' => scope_combinator(ch, chars, result),
        ':' => {
            scope_pseudo_component(chars, result, scope);
            false
        }
        '[' => {
            scope_attribute_component(chars, result);
            false
        }
        '*' => {
            result.push(ch);
            false
        }
        ch if ch.is_alphabetic() && at_start => {
            scope_element_component(ch, chars, result, scope);
            false
        }
        _ => {
            result.push(ch);
            false
        }
    }
}

#[inline]
fn scope_single_selector(selector: &str, scope: &str) -> String {
    let mut result = String::with_capacity(selector.len() + scope.len() * 4);
    let mut chars = selector.chars().peekable();
    let mut at_start = true;
    while let Some(ch) = chars.next() {
        at_start = dispatch_selector_char(ch, &mut chars, &mut result, scope, at_start);
    }
    result
}

#[inline]
fn should_preserve_space(last_ch: char) -> bool {
    !matches!(last_ch, '{' | '}' | ':' | ';' | ',')
}

#[inline]
fn should_add_space(result: &String, last_was_space: bool) -> bool {
    if last_was_space || result.is_empty() {
        return false;
    }
    match result.chars().last() {
        Some(last_ch) => should_preserve_space(last_ch),
        None => false,
    }
}

#[inline]
fn handle_minify_whitespace(result: &mut String, last_was_space: bool) -> bool {
    if should_add_space(result, last_was_space) {
        result.push(' ');
        return true;
    }
    last_was_space
}

#[inline]
fn minify_css(css: &str) -> String {
    let mut result = String::with_capacity(css.len() / 2);
    let mut chars = css.chars().peekable();
    let mut last_was_space = false;
    while let Some(ch) = chars.next() {
        if handle_comment(ch, &mut chars) {
            continue;
        }
        if ch.is_whitespace() {
            last_was_space = handle_minify_whitespace(&mut result, last_was_space);
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    result.shrink_to_fit();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_and_scope ----

    #[test]
    fn test_parse_and_scope_plain_css() {
        let result = parse_and_scope(".btn { color: red; }", "abc", false);
        assert_eq!(result, ".abc_btn { color: red; }\n");
    }

    #[test]
    fn test_parse_and_scope_media_query() {
        let css = "@media (max-width: 600px) { .btn { color: red; } }";
        let result = parse_and_scope(css, "abc", false);
        assert_eq!(
            result,
            "@media (max-width: 600px) { .abc_btn { color: red; } }\n"
        );
    }

    #[test]
    fn test_parse_and_scope_keyframes_passthrough() {
        let css = "@keyframes spin { 0% { opacity: 0; } 100% { opacity: 1; } }";
        let result = parse_and_scope(css, "abc", false);
        assert_eq!(result, format!("{}\n", css));
    }

    #[test]
    fn test_parse_and_scope_font_face_passthrough() {
        let css = "@font-face { font-family: 'Test'; }";
        let result = parse_and_scope(css, "abc", false);
        assert_eq!(result, format!("{}\n", css));
    }

    #[test]
    fn test_parse_and_scope_nested_rules_in_media() {
        let css = "@media (max-width: 600px) { .a { color: red; } .b { color: blue; } }";
        let result = parse_and_scope(css, "abc", false);
        assert!(result.contains(".abc_a { color: red; }"));
        assert!(result.contains(".abc_b { color: blue; }"));
    }

    #[test]
    fn test_parse_and_scope_minified() {
        let result = parse_and_scope(".btn { color: red; }", "abc", true);
        assert_eq!(result, ".abc_btn {color:red;}");
    }

    // ---- scope_rule ----

    #[test]
    fn test_scope_rule_empty() {
        assert_eq!(scope_rule("", "abc"), None);
        assert_eq!(scope_rule("   ", "abc"), None);
    }

    #[test]
    fn test_scope_rule_at_rule_passthrough() {
        let rule = "@keyframes spin { 0% { opacity: 0; } }";
        assert_eq!(scope_rule(rule, "abc"), Some(rule.to_string()));
    }

    #[test]
    fn test_scope_rule_normal_selector() {
        assert_eq!(
            scope_rule(".btn { color: red; }", "abc"),
            Some(".abc_btn { color: red; }".to_string())
        );
    }

    // ---- scope_selector ----

    #[test]
    fn test_scope_selector_single() {
        assert_eq!(scope_selector(".btn", "abc"), ".abc_btn");
    }

    #[test]
    fn test_scope_selector_comma_separated() {
        assert_eq!(scope_selector(".a, .b", "abc"), ".abc_a, .abc_b");
    }

    // ---- scope_single_selector ----

    #[test]
    fn test_scope_single_selector_class() {
        assert_eq!(scope_single_selector(".btn", "abc"), ".abc_btn");
    }

    #[test]
    fn test_scope_single_selector_id() {
        assert_eq!(scope_single_selector("#main", "abc"), "#main");
    }

    #[test]
    fn test_scope_single_selector_element() {
        assert_eq!(
            scope_single_selector("div", "abc"),
            "div[data-scope=\"abc\"]"
        );
    }

    #[test]
    fn test_scope_single_selector_pseudo_class() {
        assert_eq!(scope_single_selector(":hover", "abc"), ":hover");
    }

    #[test]
    fn test_scope_single_selector_pseudo_element() {
        assert_eq!(scope_single_selector("::before", "abc"), "::before");
    }

    #[test]
    fn test_scope_single_selector_attribute() {
        assert_eq!(scope_single_selector("[attr]", "abc"), "[attr]");
    }

    #[test]
    fn test_scope_single_selector_universal() {
        assert_eq!(scope_single_selector("*", "abc"), "*");
    }

    #[test]
    fn test_scope_single_selector_child_combinator() {
        assert_eq!(
            scope_single_selector("div > .btn", "abc"),
            "div[data-scope=\"abc\"] >.abc_btn"
        );
    }

    #[test]
    fn test_scope_single_selector_adjacent_combinator() {
        assert_eq!(
            scope_single_selector("div + .btn", "abc"),
            "div[data-scope=\"abc\"] +.abc_btn"
        );
    }

    #[test]
    fn test_scope_single_selector_general_sibling_combinator() {
        assert_eq!(
            scope_single_selector("div ~ .btn", "abc"),
            "div[data-scope=\"abc\"] ~.abc_btn"
        );
    }

    #[test]
    fn test_scope_single_selector_descendant_combinator() {
        assert_eq!(
            scope_single_selector("div .btn", "abc"),
            "div[data-scope=\"abc\"] .abc_btn"
        );
    }

    // ---- minify_css ----

    #[test]
    fn test_minify_css_whitespace_compression() {
        assert_eq!(minify_css(".a { color: red; }"), ".a {color:red;}");
    }

    #[test]
    fn test_minify_css_comment_removal() {
        assert_eq!(
            minify_css(".a /* c */ { color: red; }"),
            ".a {color:red;}"
        );
    }

    #[test]
    fn test_minify_css_preserve_necessary_space() {
        assert_eq!(minify_css("a b { color: red; }"), "a b {color:red;}");
    }

    // ---- parse_css_rules ----

    #[test]
    fn test_parse_css_rules_simple() {
        let rules = parse_css_rules(".a { color: red; } .b { color: blue; }");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0], ".a { color: red; }");
        assert_eq!(rules[1], ".b { color: blue; }");
    }

    #[test]
    fn test_parse_css_rules_nested_braces() {
        let rules = parse_css_rules("@media (max-width: 600px) { .a { color: red; } }");
        assert_eq!(rules.len(), 1);
        assert_eq!(
            rules[0],
            "@media (max-width: 600px) { .a { color: red; } }"
        );
    }

    #[test]
    fn test_parse_css_rules_comments() {
        let rules = parse_css_rules("/* c */ .a { color: red; }");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0], ".a { color: red; }");
    }

    #[test]
    fn test_parse_css_rules_empty() {
        let rules = parse_css_rules("");
        assert!(rules.is_empty());
    }

    // ---- skip_css_comment ----

    #[test]
    fn test_skip_css_comment_full() {
        let mut chars = "* comment */".chars().peekable();
        skip_css_comment(&mut chars);
        assert_eq!(chars.next(), None);
    }

    #[test]
    fn test_skip_css_comment_with_rest() {
        let mut chars = "* c */rest".chars().peekable();
        skip_css_comment(&mut chars);
        assert_eq!(chars.collect::<String>(), "rest");
    }

    // ---- scope_class_component ----

    #[test]
    fn test_scope_class_component_basic() {
        let mut chars = "btn".chars().peekable();
        let mut result = String::new();
        scope_class_component(&mut chars, &mut result, "abc");
        assert_eq!(result, ".abc_btn");
        assert_eq!(chars.next(), None);
    }

    #[test]
    fn test_scope_class_component_underscore_replaced() {
        let mut chars = "my_class".chars().peekable();
        let mut result = String::new();
        scope_class_component(&mut chars, &mut result, "abc");
        assert_eq!(result, ".abc_my-class");
    }

    #[test]
    fn test_scope_class_component_empty_name() {
        let mut chars = " ".chars().peekable();
        let mut result = String::new();
        scope_class_component(&mut chars, &mut result, "abc");
        assert_eq!(result, ".");
    }

    // ---- scope_id_component ----

    #[test]
    fn test_scope_id_component_basic() {
        let mut chars = "main".chars().peekable();
        let mut result = String::new();
        scope_id_component(&mut chars, &mut result);
        assert_eq!(result, "#main");
        assert_eq!(chars.next(), None);
    }

    #[test]
    fn test_scope_id_component_empty_name() {
        let mut chars = " ".chars().peekable();
        let mut result = String::new();
        scope_id_component(&mut chars, &mut result);
        assert_eq!(result, "#");
    }

    // ---- scope_element_component ----

    #[test]
    fn test_scope_element_component_basic() {
        let mut chars = "iv".chars().peekable();
        let mut result = String::new();
        scope_element_component('d', &mut chars, &mut result, "abc");
        assert_eq!(result, "div[data-scope=\"abc\"]");
    }

    #[test]
    fn test_scope_element_component_root() {
        let mut chars = "oot".chars().peekable();
        let mut result = String::new();
        scope_element_component('r', &mut chars, &mut result, "abc");
        assert_eq!(result, "root");
    }

    #[test]
    fn test_scope_element_component_host() {
        let mut chars = "ost".chars().peekable();
        let mut result = String::new();
        scope_element_component('h', &mut chars, &mut result, "abc");
        assert_eq!(result, "host");
    }

    // ---- scope_pseudo_component ----

    #[test]
    fn test_scope_pseudo_component_hover() {
        let mut chars = "hover".chars().peekable();
        let mut result = String::new();
        scope_pseudo_component(&mut chars, &mut result, "abc");
        assert_eq!(result, ":hover");
    }

    #[test]
    fn test_scope_pseudo_component_before() {
        let mut chars = ":before".chars().peekable();
        let mut result = String::new();
        scope_pseudo_component(&mut chars, &mut result, "abc");
        assert_eq!(result, "::before");
    }

    #[test]
    fn test_scope_pseudo_component_not() {
        let mut chars = "not(.x)".chars().peekable();
        let mut result = String::new();
        scope_pseudo_component(&mut chars, &mut result, "abc");
        assert_eq!(result, ":not(.abc_x)");
    }

    #[test]
    fn test_scope_pseudo_component_has() {
        let mut chars = "has(.y)".chars().peekable();
        let mut result = String::new();
        scope_pseudo_component(&mut chars, &mut result, "abc");
        assert_eq!(result, ":has(.abc_y)");
    }

    #[test]
    fn test_scope_pseudo_component_is() {
        let mut chars = "is(.a, .b)".chars().peekable();
        let mut result = String::new();
        scope_pseudo_component(&mut chars, &mut result, "abc");
        assert_eq!(result, ":is(.abc_a, .abc_b)");
    }

    // ---- scope_attribute_component ----

    #[test]
    fn test_scope_attribute_component_simple() {
        let mut chars = "attr]".chars().peekable();
        let mut result = String::new();
        scope_attribute_component(&mut chars, &mut result);
        assert_eq!(result, "[attr]");
    }

    #[test]
    fn test_scope_attribute_component_unquoted_value() {
        let mut chars = "attr=value]".chars().peekable();
        let mut result = String::new();
        scope_attribute_component(&mut chars, &mut result);
        assert_eq!(result, "[attr=value]");
    }

    #[test]
    fn test_scope_attribute_component_double_quoted_value() {
        let mut chars = "attr=\"value\"]".chars().peekable();
        let mut result = String::new();
        scope_attribute_component(&mut chars, &mut result);
        assert_eq!(result, "[attr=\"value\"]");
    }

    #[test]
    fn test_scope_attribute_component_single_quoted_value() {
        let mut chars = "attr='value']".chars().peekable();
        let mut result = String::new();
        scope_attribute_component(&mut chars, &mut result);
        assert_eq!(result, "[attr='value']");
    }
}
