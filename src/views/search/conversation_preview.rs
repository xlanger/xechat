//! 对话预览组件。
//!
//! 右侧面板，展示对话消息列表，高亮定位到选中消息。
//! 支持两种进入模式：
//! 1. 全记录点击 — 显示最新 size 条消息，滚动条置底
//! 2. 搜索匹配点击 — 定位消息置顶，显示 size 条消息
//! 滚动到上/下边界时自动触发瀑布流加载更多历史/新消息。

use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::use_search::use_search;
use crate::hooks::use_conversation;
use crate::hooks::use_app;
use crate::components::markdown::Markdown;
use crate::icons::{Icon, tabler};
use crate::MessageRole;

/// 最小触发阈值（px）
const THRESHOLD_MIN: f64 = 80.0;
/// 最大触发阈值（px）
const THRESHOLD_MAX: f64 = 200.0;
/// 触发阈值占容器高度的比例
const THRESHOLD_RATIO: f64 = 0.2;

#[with_css(css, "styles/views/search.scss")]
#[component]
pub fn ConversationPreview() -> Element {
    let search_store = use_search();
    let conv_store = use_conversation();
    let app_store = use_app();
    let tz_pref = app_store.timezone.read().clone();

    let preview_conv_id = search_store.preview_conversation_id.read().clone();
    let highlight_msg_id = search_store.highlight_message_id.read().clone();

    let mut conv_store_for_effect = conv_store.clone();
    let search_store_for_effect = search_store.clone();
    use_effect(move || {
        let preview_conv_id = search_store_for_effect.preview_conversation_id.read().clone();
        let highlight_msg_id = search_store_for_effect.highlight_message_id.read().clone();
        if let Some(conv_id) = &preview_conv_id {
            let cid = conv_id.clone();
            let anchor = highlight_msg_id.clone();
            conv_store_for_effect.reset_pagination();
            let mut store = conv_store_for_effect.clone();
            spawn(async move {
                if let Some(msg_id) = &anchor {
                    store.load_conversation_content_anchored(&cid, msg_id).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let script = format!(
                        "(function(){{\
                            var el=document.querySelector('[data-message-id=\"{}\"]');\
                            var c=document.querySelector('[data-preview-messages]');\
                            if(el&&c){{\
                                var r=el.getBoundingClientRect();\
                                var cr=c.getBoundingClientRect();\
                                c.scrollTop+=r.top-cr.top;\
                            }}\
                        }})()",
                        msg_id
                    );
                    let _ = dioxus::desktop::window().webview.evaluate_script(&script);
                } else {
                    store.load_conversation_content(&cid).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    let script = r#"
                        var container = document.querySelector('[data-preview-messages]');
                        if (container) { container.scrollTop = container.scrollHeight; }
                    "#;
                    let _ = dioxus::desktop::window().webview.evaluate_script(script);
                }
            });
        }
    });

    let conversation = preview_conv_id.as_ref().and_then(|id| {
        conv_store
            .conversations
            .read()
            .iter()
            .find(|c| c.id == *id)
            .cloned()
    });

    let pg = conv_store.message_pagination.read();
    let has_older = pg.start_index > 0;
    let has_newer = pg.end_index < pg.all_messages.len();
    let is_loading = pg.is_loading;
    drop(pg);

    // 滚动事件：检测上/下边界，自动触发加载
    let conv_store_for_scroll = conv_store.clone();
    let preview_id_for_scroll = preview_conv_id.clone();
    let onscroll = move |event: Event<ScrollData>| {
        if let Some(cid) = preview_id_for_scroll.clone() {
            let pg = conv_store_for_scroll.message_pagination.read();
            if pg.is_loading {
                return;
            }
            let can_load_older = pg.start_index > 0;
            let can_load_newer = pg.end_index < pg.all_messages.len();
            drop(pg);

            if !can_load_older && !can_load_newer {
                return;
            }

            let scroll_top = event.data().scroll_top();
            let scroll_height = event.data().scroll_height() as f64;
            let client_height = event.data().client_height() as f64;
            let max_scroll_top = (scroll_height - client_height).max(0.0);

            // 动态阈值：容器高度的 20%，限制在 [80, 200] 范围内
            let threshold = (client_height * THRESHOLD_RATIO).clamp(THRESHOLD_MIN, THRESHOLD_MAX);

            // 滚动到顶部 → 加载更早的消息
            if can_load_older && scroll_top < threshold {
                let mut store = conv_store_for_scroll.clone();
                let conv_id = cid.clone();
                // 加载前记录 scrollHeight，加载后恢复滚动位置
                let script_before = r#"
                    (function() {
                        var c = document.querySelector('[data-preview-messages]');
                        if (c) { c.dataset.prevScrollHeight = c.scrollHeight; }
                    })()
                "#;
                let _ = dioxus::desktop::window().webview.evaluate_script(script_before);
                spawn(async move {
                    store.load_more_messages_older(&conv_id).await;
                    // 恢复滚动位置：scrollTop += scrollHeight 增量
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    let script_after = r#"
                        (function() {
                            var c = document.querySelector('[data-preview-messages]');
                            if (c && c.dataset.prevScrollHeight) {
                                var prev = parseInt(c.dataset.prevScrollHeight);
                                var delta = c.scrollHeight - prev;
                                if (delta > 0) { c.scrollTop += delta; }
                                delete c.dataset.prevScrollHeight;
                            }
                        })()
                    "#;
                    let _ = dioxus::desktop::window().webview.evaluate_script(script_after);
                });
            }

            // 滚动到底部 → 加载更晚的消息
            if can_load_newer && (max_scroll_top - scroll_top) < threshold {
                let mut store = conv_store_for_scroll.clone();
                let conv_id = cid.clone();
                spawn(async move {
                    store.load_more_messages_newer(&conv_id).await;
                });
            }
        }
    };

    // 点击加载更多（用于无滚动条场景）
    let conv_store_for_click = conv_store.clone();
    let preview_id_for_click = preview_conv_id.clone();
    let on_click_load_older = move |_| {
        if let Some(cid) = preview_id_for_click.clone() {
            let mut store = conv_store_for_click.clone();
            let conv_id = cid.clone();
            let script_before = r#"
                (function() {
                    var c = document.querySelector('[data-preview-messages]');
                    if (c) { c.dataset.prevScrollHeight = c.scrollHeight; }
                })()
            "#;
            let _ = dioxus::desktop::window().webview.evaluate_script(script_before);
            spawn(async move {
                store.load_more_messages_older(&conv_id).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                let script_after = r#"
                    (function() {
                        var c = document.querySelector('[data-preview-messages]');
                        if (c && c.dataset.prevScrollHeight) {
                            var prev = parseInt(c.dataset.prevScrollHeight);
                            var delta = c.scrollHeight - prev;
                            if (delta > 0) { c.scrollTop += delta; }
                            delete c.dataset.prevScrollHeight;
                        }
                    })()
                "#;
                let _ = dioxus::desktop::window().webview.evaluate_script(script_after);
            });
        }
    };

    let conv_store_for_click_newer = conv_store.clone();
    let preview_id_for_click_newer = preview_conv_id.clone();
    let on_click_load_newer = move |_| {
        if let Some(cid) = preview_id_for_click_newer.clone() {
            let mut store = conv_store_for_click_newer.clone();
            let conv_id = cid.clone();
            spawn(async move {
                store.load_more_messages_newer(&conv_id).await;
            });
        }
    };

    match conversation {
        Some(conv) => {
            let conv_title = conv.title.clone();
            let msg_count = conv.messages.len();
            let pg = conv_store.message_pagination.read();
            let total_msg_count = pg.all_messages.len();
            let msg_count_total_str = t!("search.count-total-messages", count = msg_count.to_string(), total = total_msg_count.to_string()).to_string();
            drop(pg);

            rsx! {
                div {
                    class: "{css::preview_container}",
                    div {
                        class: "{css::preview_header}",
                        h3 { class: "{css::preview_title}", "{conv_title}" }
                        span { class: "{css::preview_meta}", "{msg_count_total_str}" }
                    }
                    div {
                        class: "{css::preview_messages}",
                        "data-preview-messages": "",
                        onscroll: onscroll,
                        if is_loading {
                            div {
                                style: "text-align:center;padding:8px;color:var(--text-secondary);font-size:12px;",
                                {t!("preview.loading").to_string()}
                            }
                        }
                        if has_older {
                            div {
                                style: "text-align:center;padding:8px;color:var(--text-secondary);font-size:12px;cursor:pointer;",
                                onclick: on_click_load_older,
                                {t!("preview.scroll-up").to_string()}
                            }
                        }
                        for msg in &conv.messages {
                            {
                                let is_highlight = highlight_msg_id.as_ref() == Some(&msg.id);
                                let is_user = msg.role == MessageRole::User;
                                let msg_id = msg.id.clone();
                                let msg_content = msg.content.clone();
                                let time_str = crate::utils::datetime::format_smart_time(&msg.timestamp, &tz_pref);
                                rsx! {
                                    div {
                                        key: "{msg_id}",
                                        "data-message-id": "{msg_id}",
                                        class: if is_highlight {
                                            "{css::preview_message} {css::preview_message_highlight}"
                                        } else {
                                            "{css::preview_message}"
                                        },
                                        div {
                                            class: "{css::preview_message_role}",
                                            if is_user {
                                                Icon { data: tabler::User, size: "14" }
                                            } else {
                                                Icon { data: tabler::Robot, size: "14" }
                                            }
                                            span { style: "margin-left:4px;font-size:12px;color:var(--text-secondary);", "{time_str}" }
                                        }
                                        Markdown { content: msg_content }
                                    }
                                }
                            }
                        }
                        if has_newer {
                            div {
                                style: "text-align:center;padding:8px;color:var(--text-secondary);font-size:12px;cursor:pointer;",
                                onclick: on_click_load_newer,
                                {t!("preview.scroll-down").to_string()}
                            }
                        }
                    }
                }
            }
        }
        None => rsx! {
            div {
                class: "{css::preview_empty}",
                {t!("preview.empty").to_string()}
            }
        },
    }
}
