//! 设置页面 - Ollama 提供商配置区段。
//!
//! 采用 Collapse 可展开样式，仅包含服务地址配置。
//! 嵌入模型选择已移至 GeneralSection 的两级联动下拉框。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_app;
use crate::components::input::Input;
use crate::components::collapse::Collapse;
use crate::icons::{Icon, tabler};

/// 探测状态枚举。
#[derive(Clone, Copy, PartialEq)]
enum ProbeStatus {
    None,
    Loading,
    Ok,
    Fail,
}

fn get_ollama_host(config: &Option<crate::models::config::XEChatConfig>) -> String {
    config.as_ref().map(|c| c.preferences.ollama.host.clone()).unwrap_or_default()
}

#[with_css(css, "styles/components/settings.scss")]
#[component]
pub fn OllamaSection() -> Element {
    let mut app_store = use_app();

    let section_text = t!("settings.ollama").to_string();
    let host_text = t!("settings.ollama-host").to_string();
    let host_placeholder = t!("settings.ollama-host-placeholder").to_string();

    // 探测状态信号
    let mut host_status: Signal<ProbeStatus> = use_signal(|| ProbeStatus::None);

    // 读取当前配置值
    let current_host = get_ollama_host(&app_store.config.read());

    // 探测 Ollama 服务地址
    {
        let host = current_host.clone();
        use_effect(move || {
            let host = host.clone();
            if host.is_empty() {
                host_status.set(ProbeStatus::None);
                return;
            }
            host_status.set(ProbeStatus::Loading);
            spawn(async move {
                let ok = crate::services::ollama::probe::probe_host(&host).await;
                host_status.set(if ok { ProbeStatus::Ok } else { ProbeStatus::Fail });
            });
        });
    }

    let host_icon = match *host_status.read() {
        ProbeStatus::Ok => Some(rsx! { Icon { data: tabler::Check, size: "16", style: "color: var(--color-success, #22c55e)" } }),
        ProbeStatus::Fail => Some(rsx! { Icon { data: tabler::X, size: "16", style: "color: var(--color-error, #ef4444)" } }),
        _ => None,
    };

    rsx! {
        section {
            class: "{css::settings_section}",
            Collapse {
                title: section_text,
                default_open: false,
                div {
                    class: "{css::provider_content}",

                    // 服务地址
                    div {
                        class: "{css::form_row}",
                        label {
                            class: "{css::form_label}",
                            "{host_text}"
                        }
                        Input {
                            value: current_host,
                            placeholder: host_placeholder,
                            right_icon: host_icon,
                            on_input: move |v: String| {
                                app_store.update_config(|config| {
                                    config.preferences.ollama.host = v;
                                });
                            },
                        }
                    }
                }
            }
        }
    }
}
