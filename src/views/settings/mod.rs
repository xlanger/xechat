//! 设置页面视图模块。
//!
//! 提供全屏设置界面，包含通用设置和模型提供商配置。
//! 所有修改实时同步到全局 config 并自动持久化到 TOML 文件。

pub mod general_section;
pub mod provider_section;
pub mod memory_section;
pub mod ollama_section;

pub use general_section::GeneralSection;
pub use provider_section::ProviderSection;
pub use memory_section::MemorySection;
pub use ollama_section::OllamaSection;

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_app, use_ui, use_conversation};
use crate::state::MainRoute;

#[with_css(css, "styles/components/settings.scss")]
#[component]
pub fn SettingsView() -> Element {
    let mut app_store = use_app();
    let mut ui_store = use_ui();
    let conv_store = use_conversation();

    use_effect(move || {
        ui_store.open_menu_id.set(None);
        ui_store.open_header_menu.set(false);
    });

    let close_settings = move |_| {
        let has_conv = conv_store.current_conversation_id.read().is_some();
        if has_conv
            && let Some(cid) = conv_store.current_conversation_id.read().as_ref() {
                app_store.navigate_to(MainRoute::Conversation(cid.clone()));
                return;
            }
        app_store.navigate_to(MainRoute::Welcome);
    };

    let title_text = t!("settings.title").to_string();
    let models_text = t!("settings.models").to_string();

    let provider_keys: Vec<String> = app_store
        .config
        .read()
        .as_ref()
        .map(|c| {
            // 1. 过滤掉不需要的 key，并将引用转为 String
            let mut keys: Vec<String> = c.model_providers.keys()
                .filter(|k| *k != "ollama" && *k != "deepseek") // 过滤掉 ollama 和 deepseek
                .cloned() // 将 &String 变成 String (因为 keys() 返回的是引用)
                .collect(); // 收集成 Vec<String>

            // 2. 在 Vec 上调用 insert，把 "deepseek" 强制放到第 0 位
            keys.insert(0, "deepseek".to_string());

            keys // 返回修改后的 Vec
        })
        .unwrap_or_default();

    rsx! {
        div {
            class: "{css::settings_page}",
            div {
                class: "{css::settings_header}",
                h2 {
                    class: "{css::settings_title}",
                    "{title_text}"
                }
                button {
                    class: "{css::settings_close_btn}",
                    onclick: close_settings,
                    "×"
                }
            }

            div {
                class: "{css::settings_content}",
                div {
                    class: "{css::settings_content_inner}",

                    GeneralSection {}

                    MemorySection {}

                    section {
                        class: "{css::settings_section}",
                        h3 {
                            class: "{css::section_title}",
                            "{models_text}"
                        }

                        OllamaSection {}

                        for pk in provider_keys {
                            ProviderSection {
                                provider_key: pk,
                            }
                        }
                    }
                }
            }
        }
    }
}
