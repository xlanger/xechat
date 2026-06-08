use regex::Regex;
use once_cell::sync::Lazy;
use crate::models::memory::{IntentResult, IntentAction, TimeRange};
use crate::Message;

static MEMORY_TRIGGERS: Lazy<Vec<Regex>> = Lazy::new(|| vec![
    Regex::new(r"(?i)(之前|上次|以前|说过|提过|记得|回顾|继续)").unwrap(),
    Regex::new(r"(?i)(帮我找|搜索|查找|有没有|在哪)").unwrap(),
    Regex::new(r"(?i)(你刚才|你之前|上文|前面提到)").unwrap(),
    Regex::new(r"(?i)(对比|比较|和之前|有什么不同|哪个更好)").unwrap(),
]);

static TIME_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| vec![
    Regex::new(r"(?i)(最近|这几天|上周|昨天|今天)").unwrap(),
    Regex::new(r"(\d{4}年?\d{1,2}月)").unwrap(),
]);

static POLITENESS_PREFIXES: &[&str] = &["请问", "能不能", "可不可以", "麻烦", "帮我"];
static TIME_WORDS: &[&str] = &["最近", "之前", "上次", "以前", "刚才"];

pub struct BuiltinIntentAnalyzer {
    memory_triggers: Vec<Regex>,
    time_patterns: Vec<Regex>,
}

impl Default for BuiltinIntentAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl BuiltinIntentAnalyzer {
    pub fn new() -> Self {
        Self {
            memory_triggers: MEMORY_TRIGGERS.to_vec(),
            time_patterns: TIME_PATTERNS.to_vec(),
        }
    }

    pub fn analyze(&self, input: &str, _recent_context: &[Message]) -> IntentResult {
        let input_lower = input.to_lowercase();

        let trigger_score: f32 = self.memory_triggers.iter()
            .map(|re| if re.is_match(&input_lower) { 1.0 } else { 0.0 })
            .sum::<f32>()
            .min(1.0);

        let time_hint = self.extract_time_hint(&input_lower);
        let time_score = match &time_hint {
            TimeRange::Any => 0.0,
            _ => 0.6,
        };

        let combined_score = (trigger_score + time_score).min(1.0);
        let memory_query = self.clean_query(input);

        let (needs_memory, confidence, action) = if combined_score > 0.5 {
            (true, 0.8 + combined_score * 0.2, IntentAction::MemoryRetrieve)
        } else {
            (false, 0.9, IntentAction::DirectQuery)
        };

        IntentResult {
            needs_memory,
            confidence,
            memory_query,
            time_hint,
            action,
        }
    }

    fn extract_time_hint(&self, input: &str) -> TimeRange {
        if self.time_patterns[0].is_match(input) {
            TimeRange::RecentDays(7)
        } else if let Some(cap) = self.time_patterns[1].captures(input) {
            TimeRange::SpecificMonth(cap[1].to_string())
        } else {
            TimeRange::Any
        }
    }

    fn clean_query(&self, input: &str) -> String {
        let mut result = input.to_string();
        for p in POLITENESS_PREFIXES {
            result = result.replace(p, "");
        }
        for t in TIME_WORDS {
            result = result.replace(t, "");
        }
        result.trim().to_string()
    }
}
