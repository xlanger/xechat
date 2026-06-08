use xechat::services::intent::BuiltinIntentAnalyzer;
use xechat::models::memory::{IntentAction, TimeRange};

#[test]
fn test_direct_query_no_trigger() {
    let analyzer = BuiltinIntentAnalyzer::new();
    let result = analyzer.analyze("什么是Rust？", &[]);
    assert!(!result.needs_memory);
    assert_eq!(result.action, IntentAction::DirectQuery);
}

#[test]
fn test_memory_trigger_before() {
    let analyzer = BuiltinIntentAnalyzer::new();
    let result = analyzer.analyze("之前我们讨论过什么？", &[]);
    assert!(result.needs_memory);
    assert_eq!(result.action, IntentAction::MemoryRetrieve);
}

#[test]
fn test_memory_trigger_last_time() {
    let analyzer = BuiltinIntentAnalyzer::new();
    let result = analyzer.analyze("上次说的那个方案怎么样了", &[]);
    assert!(result.needs_memory);
}

#[test]
fn test_memory_trigger_remember() {
    let analyzer = BuiltinIntentAnalyzer::new();
    let result = analyzer.analyze("你还记得我之前提的需求吗", &[]);
    assert!(result.needs_memory);
}

#[test]
fn test_time_hint_recent() {
    let analyzer = BuiltinIntentAnalyzer::new();
    let result = analyzer.analyze("最近有什么进展", &[]);
    assert!(result.needs_memory);
    assert!(matches!(result.time_hint, TimeRange::RecentDays(_)));
}

#[test]
fn test_clean_query_removes_politeness() {
    let analyzer = BuiltinIntentAnalyzer::new();
    let result = analyzer.analyze("请问之前说的那个功能", &[]);
    assert!(result.needs_memory);
    assert!(!result.memory_query.contains("请问"));
}

#[test]
fn test_search_trigger() {
    let analyzer = BuiltinIntentAnalyzer::new();
    let result = analyzer.analyze("帮我找一下之前关于API的讨论", &[]);
    assert!(result.needs_memory);
}
