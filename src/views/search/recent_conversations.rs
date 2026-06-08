//! 最近对话列表组件。
//!
//! 空查询时展示最近对话记录，支持分页浏览。
//! 数据通过 SearchStore 从数据库分页加载。
//! 点击对话卡片在搜索页右侧预览，不跳转到对话页。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_search::use_search;
use crate::hooks::use_app;
use crate::models::memory::SearchResult;
use crate::models::memory::SearchType;

#[with_css(css, "styles/views/search.scss")]
#[component]
pub fn RecentConversations() -> Element {
    let search_store = use_search();
    let app_store = use_app();
    let tz_pref = app_store.timezone.read().clone();

    let page = *search_store.recent_page.read();
    let total = *search_store.recent_total.read();
    let total_pages = search_store.recent_total_pages();
    let items = search_store.recent_items.read().clone();

    // use_effect 依赖 recent_page signal，page 变化时自动重新加载
    let effect_store = search_store.clone();
    use_effect(move || {
        let _page = *effect_store.recent_page.read();
        let mut store = effect_store.clone();
        spawn(async move {
            store.load_recent_conversations().await;
        });
    });

    rsx! {
        div {
            class: "{css::recent_section}",

            div {
                class: "{css::recent_header}",
                span { class: "{css::recent_title}", { t!("search.recent-conversations").to_string() } }
                span { class: "{css::recent_count}", { t!("search.count-conversations", count = total.to_string()).to_string() } }
            }

            for conv in items {
                {
                    let conv_id = conv.id.clone();
                    let conv_title = conv.title.clone();
                    let msg_count = conv.message_count;
                    let updated_str = crate::utils::datetime::format_datetime(&conv.updated_at, "short", &tz_pref);
                    let created_str = crate::utils::datetime::format_smart_time(&conv.created_at, &tz_pref);
                    let msg_count_str = t!("search.count-messages", count = msg_count.to_string()).to_string();
                    let assistant_snippet = conv.last_assistant_snippet.clone();
                    let mut store = search_store.clone();

                    let is_selected = search_store.selected_result.read().as_ref()
                        .map(|s| s.conversation_id == conv_id && s.message_id.is_empty())
                        .unwrap_or(false);
                    let item_class = if is_selected {
                        format!("{} {}", css::recent_item, css::recent_item_selected)
                    } else {
                        css::recent_item.to_string()
                    };

                    rsx! {
                        div {
                            key: "{conv_id}",
                            class: "{item_class}",
                            onclick: move |_| {
                                let result = SearchResult {
                                    conversation_id: conv_id.clone(),
                                    message_id: String::new(),
                                    title: conv_title.clone(),
                                    content_snippet: String::new(),
                                    role: String::new(),
                                    timestamp: conv.updated_at,
                                    score: 0.0,
                                    search_type: SearchType::FullText,
                                    created_at: conv.created_at,
                                    message_count: conv.message_count,
                                    last_assistant_snippet: conv.last_assistant_snippet.clone(),
                                };
                                store.select_result(result);
                            },
                            div {
                                class: "{css::recent_item_header}",
                                span { class: "{css::recent_item_title}", "{conv_title}" }
                                span { class: "{css::recent_item_time}", "{created_str}" }
                            }
                            if !assistant_snippet.is_empty() {
                                p { class: "{css::recent_item_snippet}", "{assistant_snippet}" }
                            }
                            div {
                                class: "{css::recent_item_meta_row}",
                                span { class: "{css::recent_item_meta}", "{msg_count_str}" }
                                span { class: "{css::recent_item_meta_right}", "{updated_str}" }
                            }
                        }
                    }
                }
            }

            if total_pages > 1 {
                {
                    let mut prev_store = search_store.clone();
                    let mut next_store = search_store.clone();
                    let current_page = page;
                    rsx! {
                        div {
                            class: "{css::pagination}",

                            button {
                                class: "{css::pagination_btn}",
                                disabled: current_page <= 1,
                                onclick: move |_| {
                                    if current_page > 1 {
                                        prev_store.set_recent_page(current_page - 1);
                                    }
                                },
                                "‹"
                            }

                            for p in 1..=total_pages {
                                {
                                    let mut s = search_store.clone();
                                    let is_active = p == current_page;
                                    rsx! {
                                        button {
                                            key: "{p}",
                                            class: if is_active {
                                                "{css::pagination_btn} {css::pagination_btn_active}"
                                            } else {
                                                "{css::pagination_btn}"
                                            },
                                            onclick: move |_| {
                                                s.set_recent_page(p);
                                            },
                                            "{p}"
                                        }
                                    }
                                }
                            }

                            button {
                                class: "{css::pagination_btn}",
                                disabled: current_page >= total_pages,
                                onclick: move |_| {
                                    if current_page < total_pages {
                                        next_store.set_recent_page(current_page + 1);
                                    }
                                },
                                "›"
                            }
                        }
                    }
                }
            }
        }
    }
}
