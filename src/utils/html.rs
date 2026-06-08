/// 对文本进行 HTML 实体转义，防止 XSS 注入
///
/// 将 `<`、`>`、`&`、`"`、`'` 转换为对应的 HTML 实体。
pub fn escape(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(c),
        }
    }
    result
}
