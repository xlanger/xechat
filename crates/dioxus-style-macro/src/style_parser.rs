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
fn parse_css_rules(css: &str) -> Vec<String> {
    let mut rules = Vec::with_capacity(16);
    let mut current_rule = String::with_capacity(128);
    let mut brace_count = 0;
    let mut chars = css.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            skip_css_comment(&mut chars);
            continue;
        }
        current_rule.push(ch);
        match ch {
            '{' => brace_count += 1,
            '}' => {
                brace_count -= 1;
                if brace_count == 0 {
                    let trimmed = current_rule.trim();
                    if !trimmed.is_empty() {
                        rules.push(trimmed.to_string());
                    }
                    current_rule.clear();
                }
            }
            _ => {}
        }
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
fn scope_class_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    scope: &str,
) {
    let mut class_name = String::with_capacity(16);
    while let Some(&next_ch) = chars.peek() {
        if next_ch.is_alphanumeric() || next_ch == '-' || next_ch == '_' {
            class_name.push(next_ch);
            chars.next();
        } else {
            break;
        }
    }
    if !class_name.is_empty() {
        let scoped_name = class_name.replace('_', "-");
        result.push('.');
        result.push_str(scope);
        result.push('_');
        result.push_str(&scoped_name);
    } else {
        result.push('.');
    }
}

#[inline]
fn scope_id_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
) {
    let mut id_name = String::with_capacity(16);
    while let Some(&next_ch) = chars.peek() {
        if next_ch.is_alphanumeric() || next_ch == '-' || next_ch == '_' {
            id_name.push(next_ch);
            chars.next();
        } else {
            break;
        }
    }
    if !id_name.is_empty() {
        result.push('#');
        result.push_str(&id_name);
    } else {
        result.push('#');
    }
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
fn scope_pseudo_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
    scope: &str,
) {
    result.push(':');
    if chars.peek() == Some(&':') {
        result.push(chars.next().unwrap());
    }
    let mut pseudo_name = String::new();
    while let Some(&next_ch) = chars.peek() {
        if next_ch.is_alphanumeric() || next_ch == '-' {
            pseudo_name.push(next_ch);
            result.push(next_ch);
            chars.next();
        } else {
            break;
        }
    }
    let is_functional = ["not", "has", "is", "where", "matches"]
        .iter()
        .any(|&p| pseudo_name.eq_ignore_ascii_case(p));
    if is_functional {
        if chars.peek() == Some(&'(') {
            result.push(chars.next().unwrap());
            let mut inner_content = String::new();
            let mut paren_depth = 1;
            while let Some(c) = chars.next() {
                match c {
                    '(' => {
                        paren_depth += 1;
                        inner_content.push(c);
                    }
                    ')' => {
                        paren_depth -= 1;
                        if paren_depth == 0 {
                            break;
                        }
                        inner_content.push(c);
                    }
                    _ => inner_content.push(c),
                }
            }
            let scoped_inner = scope_selector(&inner_content, scope);
            result.push_str(&scoped_inner);
            result.push(')');
        }
    }
}

#[inline]
fn scope_attribute_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    result: &mut String,
) {
    result.push('[');
    let mut in_quote: Option<char> = None;
    while let Some(next_ch) = chars.next() {
        result.push(next_ch);
        match next_ch {
            '"' | '\'' => {
                if let Some(quote_char) = in_quote {
                    if quote_char == next_ch {
                        in_quote = None;
                    }
                } else {
                    in_quote = Some(next_ch);
                }
            }
            '\\' => {
                if in_quote.is_some() {
                    if let Some(escaped) = chars.next() {
                        result.push(escaped);
                    }
                }
            }
            ']' => {
                if in_quote.is_none() {
                    break;
                }
            }
            _ => {}
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
    while let Some(&next_ch) = chars.peek() {
        if next_ch.is_alphanumeric() || next_ch == '-' {
            element_name.push(next_ch);
            chars.next();
        } else {
            break;
        }
    }
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
fn scope_single_selector(selector: &str, scope: &str) -> String {
    let mut result = String::with_capacity(selector.len() + scope.len() * 4);
    let mut chars = selector.chars().peekable();
    let mut at_start = true;

    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                scope_class_component(&mut chars, &mut result, scope);
                at_start = false;
            }
            '#' => {
                scope_id_component(&mut chars, &mut result);
                at_start = false;
            }
            ' ' | '>' | '+' | '~' => {
                at_start = scope_combinator(ch, &mut chars, &mut result);
            }
            ':' => {
                scope_pseudo_component(&mut chars, &mut result, scope);
                at_start = false;
            }
            '[' => {
                scope_attribute_component(&mut chars, &mut result);
                at_start = false;
            }
            '*' => {
                result.push(ch);
                at_start = false;
            }
            ch if ch.is_alphabetic() && at_start => {
                scope_element_component(ch, &mut chars, &mut result, scope);
                at_start = false;
            }
            _ => {
                result.push(ch);
                at_start = false;
            }
        }
    }
    result
}

#[inline]
fn should_preserve_space(last_ch: char) -> bool {
    !matches!(last_ch, '{' | '}' | ':' | ';' | ',')
}

#[inline]
fn minify_css(css: &str) -> String {
    let mut result = String::with_capacity(css.len() / 2);
    let mut chars = css.chars().peekable();
    let mut last_was_space = false;

    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            skip_css_comment(&mut chars);
            continue;
        }
        if ch.is_whitespace() {
            if !last_was_space && !result.is_empty() {
                if let Some(last_ch) = result.chars().last() {
                    if should_preserve_space(last_ch) {
                        result.push(' ');
                        last_was_space = true;
                    }
                }
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    result.shrink_to_fit();
    result
}
