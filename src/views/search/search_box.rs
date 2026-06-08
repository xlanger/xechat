//! 搜索输入框组件。
//!
//! 提供搜索查询输入和回车/点击触发搜索功能。
//! 搜索框左侧内嵌下拉选择器（搜索模式），右侧内嵌搜索图标按钮。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_search::use_search;
use crate::models::memory::SearchType;
use crate::icons::{Icon, tabler};

#[with_css(css, "styles/views/search.scss")]
#[component]
pub fn SearchInput() -> Element {
    let search_store = use_search();

    // 直接从 search_store.query 读取输入值，保持单一数据源
    let query = search_store.query.read().clone();
    let search_type = search_store.search_type.read().clone();

    let mut store_for_input = search_store.clone();
    let mut store_for_keydown = search_store.clone();
    let mut store_for_type_change = search_store.clone();
    let mut store_for_click = search_store.clone();

    rsx! {
        div {
            class: "{css::search_input_box}",
            div {
                class: "{css::search_type_select_wrapper}",
                select {
                    class: "{css::search_type_select}",
                    onchange: move |e| {
                        let new_type = match e.value().as_str() {
                            "fulltext" => SearchType::FullText,
                            "semantic" => SearchType::Semantic,
                            _ => SearchType::Hybrid,
                        };
                        store_for_type_change.set_search_type(new_type.clone());
                        let q = store_for_type_change.query.read().trim().to_string();
                        if !q.is_empty() {
                            let mut store = store_for_type_change.clone();
                            spawn(async move {
                                store.clear_selection();
                                store.execute_search().await;
                            });
                        }
                    },
                    option { value: "fulltext", selected: search_type == SearchType::FullText, { t!("search.fulltext").to_string() } }
                    option { value: "semantic", selected: search_type == SearchType::Semantic, { t!("search.semantic").to_string() } }
                    option { value: "hybrid", selected: search_type == SearchType::Hybrid, { t!("search.hybrid").to_string() } }
                }
            }
            input {
                r#type: "text",
                placeholder: t!("search.search-placeholder").to_string(),
                value: "{query}",
                class: "{css::search_input_field}",
                oninput: move |e| {
                    store_for_input.set_query(e.value());
                },
                onkeydown: move |e| {
                    if e.key() == Key::Enter {
                        e.prevent_default();
                        let q = store_for_keydown.query.read().trim().to_string();
                        if q.is_empty() {
                            store_for_keydown.clear_selection();
                        } else {
                            let mut store = store_for_keydown.clone();
                            spawn(async move {
                                store.clear_selection();
                                store.execute_search().await;
                            });
                        }
                    }
                },
            }
            button {
                class: "{css::search_submit_btn}",
                r#type: "button",
                disabled: *search_store.is_searching.read(),
                onclick: move |_| {
                    let q = store_for_click.query.read().trim().to_string();
                    if q.is_empty() {
                        store_for_click.clear_selection();
                    } else {
                        let mut store = store_for_click.clone();
                        spawn(async move {
                            store.clear_selection();
                            store.execute_search().await;
                        });
                    }
                },
                {
                    let is_searching = *search_store.is_searching.read();
                    if is_searching {
                        rsx! { Icon { data: tabler::Loader, size: "18".to_string(), style: "animation: spin 1s linear infinite" } }
                    } else {
                        rsx! { Icon { data: tabler::Search, size: "18".to_string() } }
                    }
                }
            }
        }
    }
}
