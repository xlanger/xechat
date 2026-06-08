//! 设置页面 - 记忆管线配置区段。
//!
//! 提供记忆检索最大条数的配置界面。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_app;
use crate::components::input::{Input, InputType};

fn get_max_memory_results(config: &Option<crate::models::config::XEChatConfig>) -> String {
    config.as_ref().map(|c| c.memory.max_memory_results.to_string()).unwrap_or_else(|| "5".to_string())
}

#[with_css(css, "styles/components/settings.scss")]
#[component]
pub fn MemorySection() -> Element {
    let mut app_store = use_app();

    let section_text = t!("settings.memory").to_string();
    let max_results_text = t!("settings.memory-max-results").to_string();

    rsx! {
        section {
            class: "{css::settings_section}",
            h3 {
                class: "{css::section_title}",
                "{section_text}"
            }

            div {
                class: "{css::form_row}",
                label {
                    class: "{css::form_label}",
                    "{max_results_text}"
                }
                Input {
                    value: get_max_memory_results(&app_store.config.read()),
                    placeholder: "5".to_string(),
                    input_type: InputType::Number,
                    min: Some(1.0),
                    max: Some(20.0),
                    on_input: move |v: String| {
                        if let Ok(n) = v.parse::<u32>() {
                            app_store.update_config(|config| {
                                config.memory.max_memory_results = n.clamp(1, 20);
                            });
                        }
                    },
                }
            }
        }
    }
}
