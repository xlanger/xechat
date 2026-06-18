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

enum SelectorVariant {
    Class,
    Id,
    Element,
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
fn consume_identifier(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut name = String::with_capacity(16);
    while let Some(&next) = chars.peek() {
        if next.is_alphanumeric() || next == '-' || next == '_' {
            name.push(next);
            chars.next();
        } else {
            break;
        }
    }
    name
}

#[inline]
fn register_selector(
    name: &str,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
    variant: SelectorVariant,
) {
    if name.is_empty() {
        return;
    }
    let rust_name = name.replace('-', "_");
    if seen.insert(rust_name.clone()) {
        results.push(match variant {
            SelectorVariant::Class => SelectorInfo::Class(rust_name),
            SelectorVariant::Id => SelectorInfo::Id(rust_name),
            SelectorVariant::Element => SelectorInfo::Element(rust_name),
        });
    }
}

#[inline]
fn extract_class_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) {
    let name = consume_identifier(chars);
    register_selector(&name, results, seen, SelectorVariant::Class);
}

#[inline]
fn extract_id_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) {
    let name = consume_identifier(chars);
    register_selector(&name, results, seen, SelectorVariant::Id);
}

#[inline]
fn consume_pseudo_name(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut name = String::with_capacity(16);
    while let Some(&next) = chars.peek() {
        if next.is_alphanumeric() || next == '-' {
            name.push(next);
            chars.next();
        } else {
            break;
        }
    }
    name
}

#[inline]
fn consume_paren_content(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
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
    inner
}

#[inline]
fn extract_pseudo_component(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) {
    if chars.peek() == Some(&':') {
        chars.next();
    }
    let _name = consume_pseudo_name(chars);
    if chars.peek() == Some(&'(') {
        chars.next();
        let inner = consume_paren_content(chars);
        extract_from_selector(&inner, results, seen);
    }
}

#[inline]
fn extract_attribute_component(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(c) = chars.next() {
        if c == ']' {
            break;
        }
    }
}

#[inline]
fn extract_element_component(
    first_ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) {
    let mut name = String::from(first_ch);
    name.push_str(&consume_identifier(chars));
    if name != "root" && name != "host" {
        register_selector(&name, results, seen, SelectorVariant::Element);
    }
}

#[inline]
fn dispatch_extract_char(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
    at_start: bool,
) -> bool {
    match ch {
        ' ' | '>' | '+' | '~' => true,
        '.' => {
            extract_class_component(chars, results, seen);
            false
        }
        '#' => {
            extract_id_component(chars, results, seen);
            false
        }
        ':' => {
            extract_pseudo_component(chars, results, seen);
            false
        }
        '[' => {
            extract_attribute_component(chars);
            false
        }
        '*' => false,
        ch if at_start && ch.is_alphabetic() => {
            extract_element_component(ch, chars, results, seen);
            false
        }
        _ => false,
    }
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
        at_start = dispatch_extract_char(ch, &mut chars, results, seen, at_start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect only the `Class` variant names, in order.
    fn class_names(css: &str) -> Vec<String> {
        extract_class_names(css)
            .into_iter()
            .filter_map(|s| match s {
                SelectorInfo::Class(n) => Some(n),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn single_class_selector() {
        let names = class_names(".btn { color: red; }");
        assert_eq!(names, vec!["btn".to_string()]);
    }

    #[test]
    fn multiple_classes_grouped() {
        let names = class_names(".btn, .primary { }");
        assert_eq!(names, vec!["btn".to_string(), "primary".to_string()]);
    }

    #[test]
    fn class_with_hyphen_becomes_underscore() {
        let names = class_names(".btn-primary { }");
        assert_eq!(names, vec!["btn_primary".to_string()]);
    }

    #[test]
    fn class_with_underscore_preserved() {
        let names = class_names(".btn_primary { }");
        assert_eq!(names, vec!["btn_primary".to_string()]);
    }

    #[test]
    fn id_selector_extracted() {
        let result = extract_class_names("#main { }");
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], SelectorInfo::Id(ref n) if n == "main"));
    }

    #[test]
    fn element_selector_extracted() {
        let result = extract_class_names("div { }");
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], SelectorInfo::Element(ref n) if n == "div"));
    }

    #[test]
    fn pseudo_class_hover_emits_nothing() {
        // :hover is consumed by the pseudo handler; the name itself is not emitted.
        let result = extract_class_names(":hover { }");
        assert!(result.is_empty());
    }

    #[test]
    fn combined_selector_extracts_multiple() {
        let result = extract_class_names("div.btn#main:hover { }");
        assert_eq!(result.len(), 3);
        assert!(matches!(result[0], SelectorInfo::Element(ref n) if n == "div"));
        assert!(matches!(result[1], SelectorInfo::Class(ref n) if n == "btn"));
        assert!(matches!(result[2], SelectorInfo::Id(ref n) if n == "main"));
    }

    #[test]
    fn descendant_combinator_extracts_both() {
        let names = class_names(".parent .child { }");
        assert_eq!(names, vec!["parent".to_string(), "child".to_string()]);
    }

    #[test]
    fn child_combinator_extracts_both() {
        let names = class_names(".parent > .child { }");
        assert_eq!(names, vec!["parent".to_string(), "child".to_string()]);
    }

    #[test]
    fn media_rule_skipped_entirely() {
        // The whole @media block is parsed as one rule whose selector starts with '@',
        // so it is skipped and the nested .btn is NOT extracted.
        let result = extract_class_names("@media (max-width: 768px) { .btn { } }");
        assert!(result.is_empty());
    }

    #[test]
    fn keyframes_rule_skipped() {
        let result = extract_class_names("@keyframes spin { }");
        assert!(result.is_empty());
    }

    #[test]
    fn empty_css_returns_empty() {
        let result = extract_class_names("");
        assert!(result.is_empty());
    }

    #[test]
    fn duplicate_classes_deduplicated() {
        let names = class_names(".btn { } .btn { }");
        assert_eq!(names, vec!["btn".to_string()]);
    }

    #[test]
    fn not_pseudo_extracts_inner_class() {
        let names = class_names(":not(.x) { }");
        assert_eq!(names, vec!["x".to_string()]);
    }

    #[test]
    fn has_pseudo_extracts_inner_class() {
        let names = class_names(":has(.y) { }");
        assert_eq!(names, vec!["y".to_string()]);
    }

    #[test]
    fn is_pseudo_extracts_inner_classes() {
        let names = class_names(":is(.a, .b) { }");
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn attribute_selector_extracts_nothing() {
        let result = extract_class_names("[data-x] { }");
        assert!(result.is_empty());
    }

    #[test]
    fn root_pseudo_not_extracted_as_element() {
        let result = extract_class_names(":root { }");
        assert!(result.is_empty());
    }

    #[test]
    fn host_pseudo_not_extracted_as_element() {
        let result = extract_class_names(":host { }");
        assert!(result.is_empty());
    }

    #[test]
    fn selector_info_name_for_each_variant() {
        let class = SelectorInfo::Class("foo".to_string());
        let id = SelectorInfo::Id("bar".to_string());
        let element = SelectorInfo::Element("div".to_string());
        let pseudo = SelectorInfo::Pseudo("hover".to_string());
        assert_eq!(class.name(), "foo");
        assert_eq!(id.name(), "bar");
        assert_eq!(element.name(), "div");
        assert_eq!(pseudo.name(), "hover");
    }
}
