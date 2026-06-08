//! 搜索结果列表组件。
//!
//! 展示搜索结果卡片，点击结果时调用 SearchStore 的 select_result 方法，
//! 而非路由跳转，触发右侧对话预览面板。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::models::memory::{SearchResult, SearchType};
use crate::hooks::use_search::use_search;
use crate::hooks::use_app;
use crate::icons::{Icon, tabler};

#[with_css(css, "styles/views/search.scss")]
#[component]
pub fn SearchResults(results: Vec<SearchResult>) -> Element {
    let search_store = use_search();
    let app_store = use_app();
    let tz_pref = app_store.timezone.read().clone();

    let no_results_text = t!("search.no-results").to_string();

    rsx! {
        div {
            if results.is_empty() {
                div {
                    class: "{css::search_empty}",
                    "{no_results_text}"
                }
            }
            for result in &results {
                {
                    let msg_id = result.message_id.clone();
                    let conv_id = result.conversation_id.clone();
                    let title = result.title.clone();
                    let snippet = result.content_snippet.clone();
                    let msg_count = result.message_count;
                    let msg_count_str = t!("search.count-messages", count = msg_count.to_string()).to_string();
                    let created_str = crate::utils::datetime::format_smart_time(&result.created_at, &tz_pref);
                    let type_label = match result.search_type {
                        SearchType::FullText => t!("search.fulltextmatch"),
                        SearchType::Semantic => {
                            let pct = (result.score * 100.0) as u8;
                            t!("search.semantic-related", percent = pct.to_string())
                        },
                        SearchType::Hybrid => t!("search.hybridmatch"),
                    };
                    let result_clone = result.clone();
                    let mut store = search_store.clone();

                    let is_selected = search_store.selected_result.read().as_ref()
                        .map(|s| s.conversation_id == conv_id && s.message_id == msg_id)
                        .unwrap_or(false);
                    let item_class = if is_selected {
                        format!("{} {}", css::search_result_item, css::search_result_item_selected)
                    } else {
                        css::search_result_item.to_string()
                    };

                    // 将 Q/A 格式的 snippet 渲染为带图标的结构化对话展示
                    let lines: Vec<&str> = snippet.lines().collect();
                    let mut snippet_elements = vec![];
                    for line in lines {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Some(content) = trimmed.strip_prefix("Q：").or_else(|| trimmed.strip_prefix("问：")) {
                            snippet_elements.push(rsx! {
                                div {
                                    class: "{css::search_snippet_row}",
                                    span {
                                        class: "{css::search_snippet_avatar}",
                                        Icon { data: tabler::User, size: "13" }
                                    }
                                    span { class: "{css::search_snippet_text}", "{content.trim()}" }
                                }
                            });
                        } else if let Some(content) = trimmed.strip_prefix("A：").or_else(|| trimmed.strip_prefix("答：")) {
                            snippet_elements.push(rsx! {
                                div {
                                    class: "{css::search_snippet_row}",
                                    span {
                                        class: "{css::search_snippet_avatar}",
                                        Icon { data: tabler::Robot, size: "13" }
                                    }
                                    span { class: "{css::search_snippet_text}", "{content.trim()}" }
                                }
                            });
                        } else {
                            snippet_elements.push(rsx! {
                                div {
                                    class: "{css::search_snippet_row}",
                                    span { class: "{css::search_snippet_text}", "{trimmed}" }
                                }
                            });
                        }
                    }

                    rsx! {
                        div {
                            key: "{msg_id}",
                            class: "{item_class}",
                            onclick: move |_| {
                                store.select_result(result_clone.clone());
                            },
                            div {
                                class: "{css::search_result_header}",
                                span { class: "{css::search_result_title}", "{title}" }
                                span { class: "{css::search_result_time}", "{created_str}" }
                            }
                            div {
                                class: "{css::search_snippet_content}",
                                { snippet_elements.into_iter() }
                            }
                            div {
                                class: "{css::search_result_footer}",
                                span { class: "{css::search_result_meta}", "{msg_count_str}" }
                                span { class: "{css::search_result_meta_right}", "{type_label}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
