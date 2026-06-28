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

/// Initializes the extraction state (results vector and seen set).
#[inline]
fn init_extraction_state() -> (
    Vec<SelectorInfo>,
    std::collections::HashSet<String>,
) {
    (
        Vec::with_capacity(16),
        std::collections::HashSet::with_capacity(16),
    )
}

/// Extracts selectors from a comma-separated selector string.
#[inline]
fn extract_from_comma_selectors(
    selector: &str,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) {
    for sel in selector.split(',') {
        let sel = sel.trim();
        if !sel.is_empty() {
            extract_from_selector(sel, results, seen);
        }
    }
}

/// Processes a single CSS rule: extracts selector parts and collects class names.
#[inline]
fn process_rule_selectors(
    rule: &str,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Some(selector_part) = rule.split('{').next() else {
        return;
    };
    let selector = selector_part.trim();
    if selector.starts_with('@') {
        return;
    }
    extract_from_comma_selectors(selector, results, seen);
}

pub fn extract_class_names(css: &str) -> Vec<SelectorInfo> {
    let (mut results, mut seen) = init_extraction_state();
    let rules = parse_rules(css);

    for rule in rules {
        process_rule_selectors(&rule, &mut results, &mut seen);
    }

    results
}

/// Skips a block comment body starting after `/*`.
///
/// Consumes characters until the closing `*/` is found.
#[inline]
fn skip_block_comment(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(ch) = chars.next() {
        if ch == '*' && chars.peek() == Some(&'/') {
            chars.next();
            break;
        }
    }
}

/// Attempts to handle a comment start (`/*`).
///
/// Returns `true` if `ch` begins a comment (and the comment is skipped),
/// `false` otherwise.
#[inline]
fn try_skip_comment(ch: char, chars: &mut std::iter::Peekable<std::str::Chars>) -> bool {
    if ch == '/' && chars.peek() == Some(&'*') {
        chars.next();
        skip_block_comment(chars);
        return true;
    }
    false
}

/// Finalizes the current rule buffer: trims, pushes if non-empty, then clears.
#[inline]
fn finalize_rule_buffer(current: &mut String, rules: &mut Vec<String>) {
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        rules.push(trimmed.to_string());
    }
    current.clear();
}

/// Handles a closing brace: decrements depth and finalizes rule at depth 0.
#[inline]
fn handle_close_brace(brace_depth: &mut usize, current: &mut String, rules: &mut Vec<String>) {
    *brace_depth -= 1;
    if *brace_depth == 0 {
        finalize_rule_buffer(current, rules);
    }
}

/// Processes a single character's effect on brace depth and rule finalization.
#[inline]
fn process_rule_brace(
    ch: char,
    brace_depth: &mut usize,
    current: &mut String,
    rules: &mut Vec<String>,
) {
    match ch {
        '{' => *brace_depth += 1,
        '}' => handle_close_brace(brace_depth, current, rules),
        _ => {}
    }
}

#[inline]
fn parse_rules(css: &str) -> Vec<String> {
    let mut rules = Vec::with_capacity(16);
    let mut current = String::with_capacity(128);
    let mut brace_depth = 0;
    let mut chars = css.chars().peekable();

    while let Some(ch) = chars.next() {
        if try_skip_comment(ch, &mut chars) {
            continue;
        }
        current.push(ch);
        process_rule_brace(ch, &mut brace_depth, &mut current, &mut rules);
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

/// Creates a `SelectorInfo` from a name and variant.
#[inline]
fn make_selector_info(name: String, variant: SelectorVariant) -> SelectorInfo {
    match variant {
        SelectorVariant::Class => SelectorInfo::Class(name),
        SelectorVariant::Id => SelectorInfo::Id(name),
        SelectorVariant::Element => SelectorInfo::Element(name),
    }
}

/// Deduplicates and registers a selector if not already seen.
#[inline]
fn dedup_and_register(
    name: &str,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
    variant: SelectorVariant,
) {
    let rust_name = name.replace('-', "_");
    if seen.insert(rust_name.clone()) {
        results.push(make_selector_info(rust_name, variant));
    }
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
    dedup_and_register(name, results, seen, variant);
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

/// Handles a closing parenthesis: decrements depth and returns whether group is complete.
///
/// Returns `true` if the closing paren completed the outermost group (caller should break),
/// `false` otherwise (character was pushed to `inner`).
#[inline]
fn handle_paren_close(inner: &mut String, c: char, depth: &mut usize) -> bool {
    *depth -= 1;
    if *depth == 0 {
        return true;
    }
    inner.push(c);
    false
}

/// Processes a parenthesis character, pushing to `inner` and tracking depth.
///
/// Returns `true` if the closing paren completed the outermost group (caller should break),
/// `false` otherwise.
#[inline]
fn push_paren_char(inner: &mut String, c: char, depth: &mut usize) -> bool {
    if c == '(' {
        *depth += 1;
        inner.push(c);
        return false;
    }
    if c == ')' {
        return handle_paren_close(inner, c, depth);
    }
    inner.push(c);
    false
}

#[inline]
fn consume_paren_content(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut depth = 1;
    let mut inner = String::new();
    for c in chars.by_ref() {
        if push_paren_char(&mut inner, c, &mut depth) {
            break;
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
    for c in chars.by_ref() {
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

/// Handles combinator characters (space, `>`, `+`, `~`).
///
/// Returns `Some(true)` if `ch` is a combinator (indicating the next
/// character is at the start of a new selector component), `None` otherwise.
#[inline]
fn handle_combinator_char(ch: char) -> Option<bool> {
    match ch {
        ' ' | '>' | '+' | '~' => Some(true),
        _ => None,
    }
}

/// Handles prefix-based selector characters (`.`, `#`, `:`).
///
/// Returns `Some(false)` if `ch` was handled (indicating the next character
/// is not at the start of a selector), `None` otherwise.
#[inline]
fn handle_prefix_selector_char(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
) -> Option<bool> {
    if ch == '.' {
        extract_class_component(chars, results, seen);
        return Some(false);
    }
    if ch == '#' {
        extract_id_component(chars, results, seen);
        return Some(false);
    }
    if ch == ':' {
        extract_pseudo_component(chars, results, seen);
        return Some(false);
    }
    None
}

/// Handles attribute selector start character (`[`).
///
/// Returns `Some(false)` if `ch` is `[` (attribute selector consumed),
/// `None` otherwise.
#[inline]
fn handle_bracket_char(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Option<bool> {
    if ch != '[' {
        return None;
    }
    extract_attribute_component(chars);
    Some(false)
}

/// Handles universal (`*`) and element selectors, or returns `false` as default.
#[inline]
fn handle_element_or_default(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
    at_start: bool,
) -> bool {
    if ch == '*' {
        return false;
    }
    if at_start && ch.is_alphabetic() {
        extract_element_component(ch, chars, results, seen);
    }
    false
}

#[inline]
fn dispatch_extract_char(
    ch: char,
    chars: &mut std::iter::Peekable<std::str::Chars>,
    results: &mut Vec<SelectorInfo>,
    seen: &mut std::collections::HashSet<String>,
    at_start: bool,
) -> bool {
    if let Some(r) = handle_combinator_char(ch) {
        return r;
    }
    if let Some(r) = handle_prefix_selector_char(ch, chars, results, seen) {
        return r;
    }
    if let Some(r) = handle_bracket_char(ch, chars) {
        return r;
    }
    handle_element_or_default(ch, chars, results, seen, at_start)
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

    // ---- Tests for new helper functions ----

    #[test]
    fn test_try_skip_comment_detects_start() {
        let mut chars = "* comment */rest".chars().peekable();
        assert!(try_skip_comment('/', &mut chars));
        assert_eq!(chars.collect::<String>(), "rest");
    }

    #[test]
    fn test_try_skip_comment_not_comment() {
        let mut chars = "hello".chars().peekable();
        let ch = chars.next().unwrap(); // consume 'h' as caller would
        assert!(!try_skip_comment(ch, &mut chars));
        assert_eq!(chars.next(), Some('e'));
    }

    #[test]
    fn test_process_rule_brace_open() {
        let mut current = String::new();
        let mut rules = Vec::new();
        let mut depth = 0;
        process_rule_brace('{', &mut depth, &mut current, &mut rules);
        assert_eq!(depth, 1);
        assert!(rules.is_empty());
    }

    #[test]
    fn test_process_rule_brace_close_finalizes() {
        let mut current = String::from(".btn { color: red; }");
        let mut rules = Vec::new();
        let mut depth = 1;
        process_rule_brace('}', &mut depth, &mut current, &mut rules);
        assert_eq!(depth, 0);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0], ".btn { color: red; }");
    }

    #[test]
    fn test_handle_combinator_char_space() {
        assert_eq!(handle_combinator_char(' '), Some(true));
        assert_eq!(handle_combinator_char('>'), Some(true));
        assert_eq!(handle_combinator_char('+'), Some(true));
        assert_eq!(handle_combinator_char('~'), Some(true));
    }

    #[test]
    fn test_handle_combinator_char_non_combinator() {
        assert_eq!(handle_combinator_char('.'), None);
        assert_eq!(handle_combinator_char('a'), None);
    }

    #[test]
    fn test_handle_prefix_selector_char_class() {
        let mut chars = "btn".chars().peekable();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        assert_eq!(
            handle_prefix_selector_char('.', &mut chars, &mut results, &mut seen),
            Some(false)
        );
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_handle_prefix_selector_char_id() {
        let mut chars = "main".chars().peekable();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        assert_eq!(
            handle_prefix_selector_char('#', &mut chars, &mut results, &mut seen),
            Some(false)
        );
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_handle_prefix_selector_char_pseudo() {
        let mut chars = "hover".chars().peekable();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        assert_eq!(
            handle_prefix_selector_char(':', &mut chars, &mut results, &mut seen),
            Some(false)
        );
        assert!(results.is_empty());
    }

    #[test]
    fn test_handle_prefix_selector_char_unhandled() {
        let mut chars = "abc".chars().peekable();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        assert_eq!(
            handle_prefix_selector_char('a', &mut chars, &mut results, &mut seen),
            None
        );
    }

    #[test]
    fn test_handle_bracket_char_open() {
        let mut chars = "attr]".chars().peekable();
        assert_eq!(handle_bracket_char('[', &mut chars), Some(false));
        assert_eq!(chars.next(), None);
    }

    #[test]
    fn test_handle_bracket_char_not_bracket() {
        let mut chars = "abc".chars().peekable();
        assert_eq!(handle_bracket_char('a', &mut chars), None);
    }

    #[test]
    fn test_handle_element_or_default_universal() {
        let mut chars = "".chars().peekable();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        assert!(!handle_element_or_default('*', &mut chars, &mut results, &mut seen, true));
        assert!(results.is_empty());
    }

    #[test]
    fn test_handle_element_or_default_element_at_start() {
        let mut chars = "iv".chars().peekable();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        assert!(!handle_element_or_default('d', &mut chars, &mut results, &mut seen, true));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_handle_element_or_default_element_not_at_start() {
        let mut chars = "iv".chars().peekable();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        assert!(!handle_element_or_default('d', &mut chars, &mut results, &mut seen, false));
        assert!(results.is_empty());
    }

    #[test]
    fn test_init_extraction_state() {
        let (results, seen) = init_extraction_state();
        assert!(results.is_empty());
        assert!(seen.is_empty());
    }

    #[test]
    fn test_extract_from_comma_selectors_multiple() {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        extract_from_comma_selectors(".a, .b, .c", &mut results, &mut seen);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_extract_from_comma_selectors_skips_empty() {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        extract_from_comma_selectors(".a, , .b", &mut results, &mut seen);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_process_rule_selectors_at_rule_skipped() {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        process_rule_selectors("@media (max-width: 600px) { .btn { } }", &mut results, &mut seen);
        assert!(results.is_empty());
    }

    #[test]
    fn test_process_rule_selectors_normal() {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        process_rule_selectors(".btn { color: red; }", &mut results, &mut seen);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_make_selector_info_class() {
        let info = make_selector_info("btn".to_string(), SelectorVariant::Class);
        assert!(matches!(info, SelectorInfo::Class(n) if n == "btn"));
    }

    #[test]
    fn test_make_selector_info_id() {
        let info = make_selector_info("main".to_string(), SelectorVariant::Id);
        assert!(matches!(info, SelectorInfo::Id(n) if n == "main"));
    }

    #[test]
    fn test_make_selector_info_element() {
        let info = make_selector_info("div".to_string(), SelectorVariant::Element);
        assert!(matches!(info, SelectorInfo::Element(n) if n == "div"));
    }

    #[test]
    fn test_dedup_and_register_new() {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        dedup_and_register("btn", &mut results, &mut seen, SelectorVariant::Class);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_dedup_and_register_duplicate() {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        dedup_and_register("btn", &mut results, &mut seen, SelectorVariant::Class);
        dedup_and_register("btn", &mut results, &mut seen, SelectorVariant::Class);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_dedup_and_register_hyphen_replaced() {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();
        dedup_and_register("btn-primary", &mut results, &mut seen, SelectorVariant::Class);
        assert!(matches!(&results[0], SelectorInfo::Class(n) if n == "btn_primary"));
    }

    #[test]
    fn test_push_paren_char_open() {
        let mut inner = String::new();
        let mut depth = 1;
        assert!(!push_paren_char(&mut inner, '(', &mut depth));
        assert_eq!(depth, 2);
        assert_eq!(inner, "(");
    }

    #[test]
    fn test_push_paren_char_close_not_complete() {
        let mut inner = String::new();
        let mut depth = 2;
        assert!(!push_paren_char(&mut inner, ')', &mut depth));
        assert_eq!(depth, 1);
        assert_eq!(inner, ")");
    }

    #[test]
    fn test_push_paren_char_close_complete() {
        let mut inner = String::new();
        let mut depth = 1;
        assert!(push_paren_char(&mut inner, ')', &mut depth));
        assert_eq!(depth, 0);
        assert!(inner.is_empty());
    }

    #[test]
    fn test_push_paren_char_other() {
        let mut inner = String::new();
        let mut depth = 1;
        assert!(!push_paren_char(&mut inner, 'x', &mut depth));
        assert_eq!(depth, 1);
        assert_eq!(inner, "x");
    }

    #[test]
    fn test_handle_paren_close_complete() {
        let mut inner = String::from("prev");
        let mut depth = 1;
        assert!(handle_paren_close(&mut inner, ')', &mut depth));
        assert_eq!(depth, 0);
        assert_eq!(inner, "prev");
    }

    #[test]
    fn test_handle_paren_close_not_complete() {
        let mut inner = String::from("prev");
        let mut depth = 2;
        assert!(!handle_paren_close(&mut inner, ')', &mut depth));
        assert_eq!(depth, 1);
        assert_eq!(inner, "prev)");
    }
}
