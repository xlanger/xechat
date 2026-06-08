use dioxus::prelude::*;
use dioxus_style::with_css;
use crate::views::conversation::ChatInput;

#[with_css(css, "styles/components/welcome.scss")]
/// 欢迎页视图，居中展示对话输入框，发送消息后自动跳转对话页。
#[component]
pub fn Welcome() -> Element {
    rsx! {
        div {
            class: "{css::welcome_root}",
            div {
                class: "{css::welcome_center}",
                ChatInput {}
            }
        }
    }
}
