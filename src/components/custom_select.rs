use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;

#[derive(Clone)]
struct SelectOption {
    key: String,
    label: String,
    is_selected: bool,
}

#[with_css(css, "styles/components/custom_select.scss")]
/// 自定义下拉选择组件，支持选项列表展示和选中状态同步。
#[component]
pub fn CustomSelect(options: Vec<(String, String)>, value: String, on_select: EventHandler<String>) -> Element {
    let mut open = use_signal(|| false);
    let selected_label = options.iter()
        .find(|(k, _)| k == &value)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| if value.is_empty() { t!("common.please-select").to_string() } else { value.clone() });
    let is_placeholder = value.is_empty();

    let toggle = move |e: MouseEvent| {
        e.stop_propagation();
        let is = *open.read();
        open.set(!is);
    };

    let mut select_item = {
        let mut open = open;
        move |v: String| {
            on_select.call(v.clone());
            open.set(false);
        }
    };

    let is_open = *open.read();

    let trigger_class = if is_open {
        format!("{} {}", css::select_trigger, css::select_trigger_open)
    } else if !is_placeholder {
        format!("{} {}", css::select_trigger, css::select_trigger_active)
    } else {
        format!("{}", css::select_trigger)
    };

    let arrow_class = if is_open {
        format!("{} {}", css::select_arrow, css::select_arrow_open)
    } else {
        format!("{}", css::select_arrow)
    };

    let select_options: Vec<SelectOption> = options.iter()
        .map(|(k, v)| SelectOption {
            key: k.clone(),
            label: v.clone(),
            is_selected: k == &value,
        })
        .collect();

    rsx! {
        div {
            class: "{css::select_wrapper}",
            tabindex: "0",
            onfocusout: move |_| open.set(false),
            div {
                class: "{trigger_class}",
                onclick: toggle,
                span { class: "{css::select_trigger_label}",
                    "{selected_label}"
                }
                span {
                    class: "{arrow_class}",
                    "\u{25BC}"
                }
            }
            {
                if is_open && !options.is_empty() {
                    rsx! {
                        div {
                            class: "{css::select_dropdown}",
                            for opt in &select_options {
                                div {
                                    class: if opt.is_selected {
                                        format!("{} {}", css::select_option, css::select_option_selected)
                                    } else {
                                        format!("{}", css::select_option)
                                    },
                                    onclick: {
                                        let k = opt.key.clone();
                                        move |_| select_item(k.clone())
                                    },
                                    "{opt.label}"
                                }
                            }
                        }
                    }
                } else {
                    rsx! { {} }
                }
            }
        }
    }
}