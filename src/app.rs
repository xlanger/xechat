use dioxus::prelude::*;
use crate::state::ThemeMode;
use crate::hooks::{use_app_provider, use_conversation_provider, use_ui_provider, use_ui, use_search_provider};
use crate::views::Layout;
use crate::components::notification::Notification;
use crate::components::modals::rename::RenameModal;
use crate::components::modals::delete::DeleteModal;

/// 应用根组件
///
/// 负责初始化全局状态、应用主题模式，
/// 并组装布局和各类模态框。
///
/// 后端初始化（嵌入器、LanceDB）由 `use_conversation_provider` 内部自动完成，
/// 本组件不再直接调用 service 层方法。
#[component]
pub fn App() -> Element {
    let app_store = use_app_provider();
    let _conversation_store = use_conversation_provider();
    let _ui_store = use_ui_provider();
    use_search_provider();

    let theme_attr = {
        let mode = *app_store.theme_mode.read();
        match mode {
            ThemeMode::System => "dark".to_string(),
            ThemeMode::Dark => "dark".to_string(),
            ThemeMode::Light => "light".to_string(),
        }
    };

    let mut ui_store = use_ui();
    let close_all_menus = move |_| {
        ui_store.open_menu_id.set(None);
        ui_store.open_header_menu.set(false);
    };

    rsx! {
        div {
            "data-theme": "{theme_attr}",
            class: "app-root",
            onclick: close_all_menus,
            Layout {}
            Notification {}
            RenameModal {}
            DeleteModal {}
        }
    }
}
