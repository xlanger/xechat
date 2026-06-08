use xechat::utils::html::escape;

#[test]
fn test_escape_lt() {
    assert_eq!(escape("<"), "&lt;");
}

#[test]
fn test_escape_gt() {
    assert_eq!(escape(">"), "&gt;");
}

#[test]
fn test_escape_amp() {
    assert_eq!(escape("&"), "&amp;");
}

#[test]
fn test_escape_double_quote() {
    assert_eq!(escape("\""), "&quot;");
}

#[test]
fn test_escape_single_quote() {
    assert_eq!(escape("'"), "&#39;");
}

#[test]
fn test_escape_plain_text() {
    assert_eq!(escape("hello world"), "hello world");
}

#[test]
fn test_escape_mixed() {
    assert_eq!(
        escape("<div class=\"foo\">&bar's</div>"),
        "&lt;div class=&quot;foo&quot;&gt;&amp;bar&#39;s&lt;/div&gt;"
    );
}

#[test]
fn test_escape_empty() {
    assert_eq!(escape(""), "");
}

#[test]
fn test_escape_multiple_same_chars() {
    assert_eq!(escape("<<<"), "&lt;&lt;&lt;");
}

#[test]
fn test_escape_unicode() {
    assert_eq!(escape("你好<世界>"), "你好&lt;世界&gt;");
}
