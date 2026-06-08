use xechat::stores::conversation::parse_first_response;

#[test]
fn test_parse_first_response_with_title() {
    let content = "[TITLE:Rust 所有权机制]\n\nRust 的所有权系统是其最独特的特性...";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, Some("Rust 所有权机制".to_string()));
    assert_eq!(body, "Rust 的所有权系统是其最独特的特性...");
}

#[test]
fn test_parse_first_response_without_title() {
    let content = "这是一个没有标题格式的普通回复。";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, None);
    assert_eq!(body, content);
}

#[test]
fn test_parse_first_response_title_not_at_start() {
    let content = "先有一些内容 [TITLE:不应解析] 后面还有内容。";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, None);
    assert_eq!(body, content);
}

#[test]
fn test_parse_first_response_empty_title() {
    let content = "[TITLE:]\n\n实际回复内容。";
    let (title, body) = parse_first_response(content);
    assert_eq!(title, Some("".to_string()));
    assert_eq!(body, "实际回复内容。");
}
