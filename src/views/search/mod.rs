//! 搜索视图模块，实现搜索页面的核心交互。
//!
//! 两阶段布局：
//! - 空查询：搜索框居中 + 最近对话列表（分页）
//! - 有搜索结果：搜索框 + 结果列表全宽
//! - 选中结果后：左侧 40% 搜索结果列表 + 右侧 60% 对话预览

pub mod search_box;
pub mod search_results;
pub mod conversation_preview;
pub mod recent_conversations;

use dioxus::prelude::*;
use dioxus_style::with_css;
use crate::hooks::use_search::use_search;

#[with_css(css, "styles/views/search.scss")]
#[component]
pub fn SearchView() -> Element {
    let search_store = use_search();

    let query = search_store.query.read().clone();
    let is_empty_query = query.trim().is_empty();

    let results = search_store.results.read().clone();
    let has_selection = search_store.selected_result.read().is_some();

    rsx! {
        div {
            class: if has_selection {
                "{css::search_view}"
            } else {
                "{css::search_view} {css::search_view_column}"
            },

            div {
                class: if has_selection {
                    "{css::search_panel} {css::search_panel_with_preview}"
                } else {
                    "{css::search_panel}"
                },

                div {
                    class: "{css::search_input_wrapper}",
                    div {
                        class: "{css::search_input_container}",
                        search_box::SearchInput {}
                    }
                }

                div {
                    class: "{css::search_results_wrapper}",
                    if is_empty_query {
                        recent_conversations::RecentConversations {}
                    } else {
                        search_results::SearchResults { results }
                    }
                }
            }

            if has_selection {
                div {
                    class: "{css::search_preview_panel}",
                    conversation_preview::ConversationPreview {}
                }
            }
        }
    }
}
