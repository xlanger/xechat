//! 侧边栏底部组件模块。
//!
//! 提供搜索入口、主题切换菜单、语言切换按钮和设置入口。
//! 本模块属于 components 层，通过 hooks 获取 store。

use dioxus::prelude::*;
use dioxus_style::with_css;
use crate::hooks::use_app;
use crate::state::{ThemeMode, MainRoute};

use crate::icons::{Icon, tabler};

#[with_css(css, "styles/components/sidebar.scss")]
/// 侧边栏底部工具栏组件，提供搜索入口、主题切换和设置入口。
#[component]
pub fn SidebarFooter() -> Element {
    let app_store = use_app();
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
    let icon_node = match current_mode {
        ThemeMode::Light => rsx! { Icon { data: tabler::Sun, size: "16" } },
        ThemeMode::Dark => rsx! { Icon { data: tabler::Moon, size: "16" } },
        _ => rsx! { Icon { data: tabler::SunMoon, size: "16" } },
    };
    rsx! {
        div {
            class: "{css::sidebar_footer}",
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
