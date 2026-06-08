use dioxus::prelude::*;
use dioxus_style::with_css;
use crate::icons::{Icon, tabler};

#[with_css(css, "styles/components/collapse.scss")]
/// 可折叠面板组件。
///
/// 点击标题栏切换内容区域的显示/隐藏状态，
/// 用于设置页面中的模型提供商卡片等场景。
///
/// # 示例
///
/// ```ignore
/// rsx! {
///     Collapse {
///         title: "高级设置",
///         default_open: false,
///         div { "这里是可折叠的内容" }
///     }
/// }
/// ```
#[component]
pub fn Collapse(
    /// 面板标题
    title: String,
    /// 默认是否展开
    #[props(default = true)]
    default_open: bool,
    /// 面板内容
    children: Element,
) -> Element {
    let mut is_open = use_signal(|| default_open);

    let toggle = move |_| {
        let current = *is_open.read();
        is_open.set(!current);
    };

    let open = *is_open.read();

    let header_class = if open {
        format!("{} {}", css::collapse_header, css::collapse_header_open)
    } else {
        format!("{}", css::collapse_header)
    };

    let icon_class = if open {
        format!("{} {}", css::collapse_icon, css::collapse_icon_open)
    } else {
        format!("{}", css::collapse_icon)
    };

    let content_class = if open {
        format!("{} {}", css::collapse_content, css::collapse_content_open)
    } else {
        format!("{}", css::collapse_content)
    };

    rsx! {
        div {
            class: "{css::collapse_wrapper}",
            div {
                class: "{header_class}",
                onclick: toggle,
                span {
                    class: "{css::collapse_title}",
                    "{title}"
                }
                span {
                    class: "{icon_class}",
                    Icon { data: tabler::ChevronDown, size: "14" }
                }
            }
            div {
                class: "{content_class}",
                {children}
            }
        }
    }
}
