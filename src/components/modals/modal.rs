use dioxus::prelude::*;
use dioxus_style::with_css;

#[derive(Props, Clone, PartialEq)]
pub struct ModalProps {
    pub title: String,
    pub onclose: EventHandler<()>,
    #[props(default = false)]
    pub show: bool,
    pub children: Element,
}

/// 通用模态框容器组件。
///
/// 提供标题栏（含关闭按钮）、内容区和可选底部区域的模态弹窗。
/// 通过 `show` 属性控制显隐，点击遮罩层或关闭按钮触发 `onclose`。
#[with_css(css, "styles/components/modals/modal.scss")]
#[component]
pub fn Modal(props: ModalProps) -> Element {
    if !props.show {
        return rsx! { {} };
    }

    let close_header = move |_| props.onclose.call(());

    rsx! {
        div {
            class: "{css::modal_overlay}",
            onclick: move |_| props.onclose.call(()),
            div {
                class: "{css::modal_dialog}",
                onclick: |e| e.stop_propagation(),
                div {
                    class: "{css::modal_header}",
                    h3 {
                        class: "{css::modal_header_title}",
                        "{props.title}"
                    }
                    span {
                        class: "{css::modal_header_close}",
                        onclick: close_header,
                        "\u{2715}"
                    }
                }
                div {
                    class: "{css::modal_body}",
                    {props.children}
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct ModalFooterProps {
    pub children: Element,
}

#[with_css(css, "styles/components/modals/modal.scss")]
#[component]
pub fn ModalFooter(props: ModalFooterProps) -> Element {
    rsx! {
        div {
            class: "{css::modal_footer}",
            {props.children}
        }
    }
}