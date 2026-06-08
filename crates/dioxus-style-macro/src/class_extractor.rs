//! CSS class name extraction for generating Rust constants.

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum SelectorInfo {
    Class(String),
    Id(String),
    Element(String),
    Pseudo(String),
}

impl SelectorInfo {
    pub fn name(&self) -> &str {
        match self {
            SelectorInfo::Class(n) | SelectorInfo::Id(n) | SelectorInfo::Element(n) | SelectorInfo::Pseudo(n) => n,
        }
    }
}

pub fn extract_class_names(css: &str) -> Vec<SelectorInfo> {
    let mut results = Vec::with_capacity(16);
    let mut seen = std::collections::HashSet::with_capacity(16);
    let rules = parse_rules(css);

    for rule in rules {
        if let Some(selector_part) = rule.split('{').next() {
            let selector = selector_part.trim();
            if selector.starts_with('@') {
                continue;
            }
            for sel in selector.split(',') {
                let sel = sel.trim();
                if sel.is_empty() {
                    continue;
                }
                extract_from_selector(sel, &mut results, &mut seen);
            }
        }
    }

    results
}

#[inline]
fn parse_rules(css: &str) -> Vec<String> {
    let mut rules = Vec::with_capacity(16);
    let mut current = String::with_capacity(128);
    let mut brace_depth = 0;
    let mut in_comment = false;
    let mut chars = css.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_comment = false;
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_comment = true;
            continue;
        }
        current.push(ch);
        match ch {
            '{' => brace_depth += 1,
            '}' => {
                brace_depth -= 1;
                if brace_depth == 0 {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        rules.push(trimmed.to_string());
                    }
                    current.clear();
                }
            }
            _ => {}
        }
    }
    rules
}

#[inline]
fn extract_from_selector(
    selector: &str,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) {
    let mut chars = selector.chars().peekable();
    let mut at_start = true;

    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                let mut name = String::with_capacity(16);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '-' || next == '_' {
                        name.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !name.is_empty() {
                    let rust_name = name.replace('-', "_");
                    if seen.insert(rust_name.clone()) {
                        results.push(SelectorInfo::Class(rust_name));
                    }
                }
                at_start = false;
            }
            '#' => {
                let mut name = String::with_capacity(16);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '-' || next == '_' {
                        name.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !name.is_empty() {
                    let rust_name = name.replace('-', "_");
                    if seen.insert(rust_name.clone()) {
                        results.push(SelectorInfo::Id(rust_name));
                    }
                }
                at_start = false;
            }
            ':' => {
                let mut name = String::with_capacity(16);
                if chars.peek() == Some(&':') {
                    chars.next();
                }
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '-' {
                        name.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() == Some(&'(') {
                    chars.next();
                    let mut depth = 1;
                    let mut inner = String::new();
                    while let Some(c) = chars.next() {
                        match c {
                            '(' => {
                                depth += 1;
                                inner.push(c);
                            }
                            ')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                inner.push(c);
                            }
                            _ => inner.push(c),
                        }
                    }
                    extract_from_selector(&inner, results, seen);
                }
                at_start = false;
            }
            ' ' | '>' | '+' | '~' => {
                at_start = true;
            }
            '[' => {
                while let Some(c) = chars.next() {
                    if c == ']' {
                        break;
                    }
                }
                at_start = false;
            }
            '*' => {
                at_start = false;
            }
            ch if ch.is_alphabetic() && at_start => {
                let mut name = String::from(ch);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '-' {
                        name.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if name != "root" && name != "host" {
                    let rust_name = name.replace('-', "_");
                    if seen.insert(rust_name.clone()) {
                        results.push(SelectorInfo::Element(rust_name));
                    }
                }
                at_start = false;
            }
            _ => {
                at_start = false;
            }
        }
    }
}
