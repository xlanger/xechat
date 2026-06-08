use xechat::models::i18n::Language;

#[test]
fn test_to_locale_zh() {
    assert_eq!(Language::Zh.to_locale(), "zh-CN");
}

#[test]
fn test_to_locale_en() {
    assert_eq!(Language::En.to_locale(), "en");
}

#[test]
fn test_to_locale_system_returns_valid_locale() {
    let locale = Language::System.to_locale();
    assert!(
        locale == "zh-CN" || locale == "en",
        "System locale should be zh-CN or en, got: {}",
        locale
    );
}

#[test]
fn test_from_locale_zh() {
    assert_eq!(Language::from_locale("zh"), Some(Language::Zh));
    assert_eq!(Language::from_locale("zh-CN"), Some(Language::Zh));
    assert_eq!(Language::from_locale("zh-TW"), Some(Language::Zh));
}

#[test]
fn test_from_locale_en() {
    assert_eq!(Language::from_locale("en"), Some(Language::En));
    assert_eq!(Language::from_locale("en-US"), Some(Language::En));
    assert_eq!(Language::from_locale("en-GB"), Some(Language::En));
}

#[test]
fn test_from_locale_unknown_returns_none() {
    assert_eq!(Language::from_locale("fr"), None);
    assert_eq!(Language::from_locale("ja"), None);
    assert_eq!(Language::from_locale("de"), None);
    assert_eq!(Language::from_locale(""), None);
}

#[test]
fn test_from_locale_case_sensitive() {
    assert_eq!(Language::from_locale("ZH"), None);
    assert_eq!(Language::from_locale("EN"), None);
    assert_eq!(Language::from_locale("Zh-CN"), None);
}

#[test]
fn test_language_equality() {
    assert_eq!(Language::Zh, Language::Zh);
    assert_eq!(Language::En, Language::En);
    assert_eq!(Language::System, Language::System);
    assert_ne!(Language::Zh, Language::En);
    assert_ne!(Language::System, Language::Zh);
}

#[test]
fn test_language_is_copy() {
    let lang = Language::Zh;
    let copied = lang;
    assert_eq!(lang, copied);
}

#[test]
fn test_roundtrip_zh_locale() {
    let locale = Language::Zh.to_locale();
    assert_eq!(Language::from_locale(locale), Some(Language::Zh));
}

#[test]
fn test_roundtrip_en_locale() {
    let locale = Language::En.to_locale();
    assert_eq!(Language::from_locale(locale), Some(Language::En));
}
