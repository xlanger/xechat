//! 侧边栏底部组件模块。
//!
//! 提供搜索入口、主题切换菜单、语言切换按钮、设置入口
//! 以及网络状态和嵌入模型状态图标按钮。
//! 本模块属于 components 层，通过 hooks 获取 store。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_app, use_conversation};
use crate::state::{ThemeMode, MainRoute};

use crate::icons::{Icon, tabler};
use crate::components::tooltip::Tooltip;

/// 根据主题模式返回对应的图标节点。
///
/// - Light → Sun
/// - Dark → Moon
/// - System → SunMoon
pub fn render_theme_icon(current_mode: ThemeMode) -> Element {
    match current_mode {
        ThemeMode::Light => rsx! { Icon { data: tabler::Sun, size: "16" } },
        ThemeMode::Dark => rsx! { Icon { data: tabler::Moon, size: "16" } },
        _ => rsx! { Icon { data: tabler::SunMoon, size: "16" } },
    }
}

/// 构建网络状态按钮的 CSS 类名。
#[inline]
pub fn network_btn_class(network_ok: bool, base_class: dioxus_style::CssClass, disabled_class: dioxus_style::CssClass) -> dioxus_style::CssClass {
    if network_ok {
        base_class
    } else {
        base_class + disabled_class
    }
}

#[with_css(css, "styles/components/sidebar.scss")]
/// 侧边栏底部工具栏组件，提供搜索入口、主题切换和设置入口。
///
/// 状态图标行为：
/// - **网络图标**：不可用时显示 WifiOff（禁用态 + 常驻 tooltip），
///   可用时显示 Wifi（正常态 + hover tooltip）
/// - **模型图标**：未就绪时显示 Download（禁用态 + 常驻 tooltip，点击跳转设置），
///   就绪时隐藏
#[component]
pub fn SidebarFooter() -> Element {
    let app_store = use_app();
    let conv_store = use_conversation();
    let mut is_theme_menu_open = use_signal(|| false);

    let open_search = move |_| {
        let mut app_store = app_store;
        app_store.navigate_to(MainRoute::Search);
    };

    let open_config = move |_| {
        let mut app_store = app_store;
        app_store.navigate_to(MainRoute::Settings);
    };

    let toggle_theme_menu = move |e: MouseEvent| {
        e.stop_propagation();
        is_theme_menu_open.toggle();
    };

    let set_theme = {
        let mut app_store = app_store;
        move |mode: ThemeMode| {
            move |_| {
                app_store.set_theme_mode(mode);
                is_theme_menu_open.set(false);
            }
        }
    };

    let current_mode = *app_store.theme_mode.read();
    let menu_open = *is_theme_menu_open.read();
    let network_ok = *app_store.network_available.read();
    let embedder_ok = *conv_store.embedder_ready.read();

    // 网络状态 tooltip 文本
    let network_tooltip = if network_ok {
        t!("status.network-ok").to_string()
    } else {
        t!("status.network-unavailable").to_string()
    };
    // 模型状态 tooltip 文本
    let model_tooltip = t!("status.embed-model-not-ready").to_string();

    let icon_node = render_theme_icon(current_mode);
    let network_class = network_btn_class(network_ok, css::sidebar_footer_btn, css::sidebar_footer_btn_disabled);
    rsx! {
        div {
            class: "{css::sidebar_footer}",
            // 网络状态图标
            Tooltip {
                text: network_tooltip,
                div {
                    class: "{network_class}",
                    if network_ok {
                        Icon { data: tabler::Wifi, size: "18" }
                    } else {
                        Icon { data: tabler::WifiOff, size: "18" }
                    },
                }
            }
            // 模型状态图标（仅未就绪时显示）
            if !embedder_ok {
                Tooltip {
                    text: model_tooltip,
                    div {
                        class: "{css::sidebar_footer_btn} {css::sidebar_footer_btn_disabled}",
                        onclick: open_config,
                        Icon { data: tabler::Download, size: "18" }
                    }
                }
            }
            div {
                class: "{css::sidebar_footer_btn_settings}",
                onclick: open_config,
                Icon { data: tabler::Settings, size: "20" }
            }
            div {
                class: "{css::sidebar_footer_btn}",
                onclick: toggle_theme_menu,
                {icon_node}
                if menu_open {
                    div {
                        class: "{css::sidebar_theme_menu}",
                        onclick: |e| e.stop_propagation(),
                        div {
                            class: "{css::sidebar_theme_option}",
                            onclick: set_theme(ThemeMode::System),
                            Icon { data: tabler::SunMoon, size: "16" }
                            span { "跟随系统" }
                        }
                        div {
                            class: "{css::sidebar_theme_option}",
                            onclick: set_theme(ThemeMode::Dark),
                            Icon { data: tabler::Moon, size: "15" }
                            span { "暗色模式" }
                        }
                        div {
                            class: "{css::sidebar_theme_option}",
                            onclick: set_theme(ThemeMode::Light),
                            Icon { data: tabler::Sun, size: "15" }
                            span { "亮色模式" }
                        }
                    }
                }
            }
            div {
                class: "{css::sidebar_footer_btn}",
                onclick: open_search,
                Icon { data: tabler::Search, size: "20" }
            }
        }
    }
}
