/// ASCII 特殊字符到 HTML 实体的静态查找表。
const HTML_ENTITIES: [(char, &str); 5] = [
    ('<', "&lt;"),
    ('>', "&gt;"),
    ('&', "&amp;"),
    ('"', "&quot;"),
    ('\'', "&#39;"),
];

/// 将特殊字符映射为 HTML 实体，返回 `Some(entity)` 或 `None`（普通字符）。
fn char_to_entity(c: char) -> Option<&'static str> {
    HTML_ENTITIES.iter().find(|(ch, _)| *ch == c).map(|(_, entity)| *entity)
}

/// 对单个字符进行 HTML 实体转义，写入结果字符串。
fn write_escaped_char(c: char, result: &mut String) {
    match char_to_entity(c) {
        Some(entity) => result.push_str(entity),
        None => result.push(c),
    }
}

/// 对文本进行 HTML 实体转义，防止 XSS 注入
///
/// 将 `<`、`>`、`&`、`"`、`'` 转换为对应的 HTML 实体。
pub fn escape(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        write_escaped_char(c, &mut result);
    }
    result
}
