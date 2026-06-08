use dioxus::prelude::*;
use crate::utils::markdown::{render_to_html, SyntaxTheme, mermaid_js};
use crate::hooks::use_app;
use crate::state::ThemeMode;

#[derive(Props, Clone, PartialEq)]
pub struct MarkdownProps {
    pub content: String,
}

#[component]
pub fn Markdown(props: MarkdownProps) -> Element {
    let app_store = use_app();
    let theme_mode = app_store.theme_mode.read();

    let syntax_theme = match *theme_mode {
        ThemeMode::Dark => SyntaxTheme::Dark,
        ThemeMode::Light => SyntaxTheme::Light,
        ThemeMode::System => SyntaxTheme::Dark,
    };

    let (html, _diag_panic) = std::panic::catch_unwind(|| render_to_html(&props.content, syntax_theme))
        .map(|ok| (ok, None))
        .unwrap_or_else(|e| {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            (format!("<p style='color:red'>[RENDER ERROR] {}</p>", msg), Some(msg))
        });

    // 注入 mermaid 交互 JS（dangerous_inner_html 中的 <script> 不会执行）
    // 只在检测到 mermaid 块时注入一次，避免流式输出时频繁 evaluate_script 干扰 DOM
    let has_mermaid = html.contains("mermaid-block");
    use_effect(use_reactive!(|has_mermaid| {
        if has_mermaid {
            let _ = dioxus::desktop::window().webview.evaluate_script(mermaid_js());
        }
    }));

    rsx! {
        div {
            style: "white-space: normal; line-height: 1.2; color: var(--text);",
            dangerous_inner_html: "{html}"
        }
    }
}
