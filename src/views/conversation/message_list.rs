//! 对话消息列表组件。
//!
//! 展示当前对话的所有消息，支持滚动到顶部/底部时自动加载更早/更晚的消息。
//! 流式响应时自动滚动到底部。
//! 加载更早消息后保持滚动位置，避免页面闪跳。

use dioxus::prelude::*;
use dioxus_style::with_css;
use crate::hooks::use_conversation;
use crate::MessageRole;
use crate::MessageStatus;
use super::message_bubble::MessageBubble;

/// 最小触发阈值（px）
const THRESHOLD_MIN: f64 = 80.0;
/// 最大触发阈值（px）
const THRESHOLD_MAX: f64 = 200.0;
/// 触发阈值占容器高度的比例
const THRESHOLD_RATIO: f64 = 0.2;

/// 计算动态滚动阈值，限制在 [THRESHOLD_MIN, THRESHOLD_MAX] 范围内。
#[inline]
pub fn compute_scroll_threshold(client_height: f64) -> f64 {
    (client_height * THRESHOLD_RATIO).clamp(THRESHOLD_MIN, THRESHOLD_MAX)
}

/// 判断是否应加载更早的消息。
#[inline]
pub fn should_load_older(can_load: bool, scroll_top: f64, threshold: f64) -> bool {
    can_load && scroll_top < threshold
}

/// 判断是否应加载更晚的消息。
#[inline]
pub fn should_load_newer(can_load: bool, max_scroll_top: f64, scroll_top: f64, threshold: f64) -> bool {
    can_load && (max_scroll_top - scroll_top) < threshold
}

/// 判断是否显示流式消息气泡。
#[inline]
pub fn should_show_streaming(is_streaming: bool, has_real_assistant: bool, stream_content: &str) -> bool {
    is_streaming && !has_real_assistant && !stream_content.trim().is_empty()
}

/// 检查最后一条消息是否为有内容的助手消息。
#[inline]
pub fn has_real_assistant_message(messages: &[crate::Message]) -> bool {
    messages.last()
        .map(|m| m.role == MessageRole::Assistant && !m.content.is_empty())
        .unwrap_or(false)
}

#[with_css(css, "styles/components/conversation.scss")]
#[component]
pub fn MessageList() -> Element {
    let conv_store = use_conversation();

    let scroll_to_bottom = || {
        spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let _ = dioxus::desktop::window().webview.evaluate_script(
                "var c=document.querySelector('[data-scroll-messages]');if(c){c.scrollTop=c.scrollHeight;}"
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
            let _ = dioxus::desktop::window().webview.evaluate_script(
                "var c=document.querySelector('[data-scroll-messages]');if(c){c.scrollTop=c.scrollHeight;}"
            );
        });
    };

    use_effect(move || {
        let _ = conv_store.current_conversation_id.read();
        let _ = conv_store.conversations.read();
        scroll_to_bottom();
    });

    use_effect(move || {
        let _ = conv_store.streaming_content.read();
        scroll_to_bottom();
    });

    let current_id = conv_store.current_conversation_id.read().clone();
    let messages = match current_id.clone() {
        Some(id) => {
            let convs = conv_store.conversations.read();
            convs.iter()
                .find(|c| c.id == id)
                .map(|c| c.messages.clone())
                .unwrap_or_default()
        }
        None => vec![],
    };

    let is_streaming = *conv_store.is_streaming.read();
    let stream_content = conv_store.streaming_content.read().clone();
    let stream_reasoning = conv_store.streaming_reasoning.read().clone();

    let has_real_assistant = has_real_assistant_message(&messages);

    let show_streaming = should_show_streaming(is_streaming, has_real_assistant, &stream_content);

    let pg = conv_store.message_pagination.read();
    let has_older = pg.start_index > 0;
    let has_newer = pg.end_index < pg.all_messages.len();
    let is_loading = pg.is_loading;
    drop(pg);

    let conv_store_for_scroll = conv_store.clone();
    let onscroll = move |event: Event<ScrollData>| {
        if let Some(cid) = current_id.clone() {
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
            let threshold = compute_scroll_threshold(client_height);

            // 滚动到顶部 → 加载更早的消息
            if should_load_older(can_load_older, scroll_top, threshold) {
                let mut store = conv_store_for_scroll.clone();
                let conv_id = cid.clone();
                // 加载前记录 scrollHeight，加载后恢复滚动位置
                let script_before = r#"
                    (function() {
                        var c = document.querySelector('[data-scroll-messages]');
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
                            var c = document.querySelector('[data-scroll-messages]');
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
            if should_load_newer(can_load_newer, max_scroll_top, scroll_top, threshold) {
                let mut store = conv_store_for_scroll.clone();
                let conv_id = cid.clone();
                spawn(async move {
                    store.load_more_messages_newer(&conv_id).await;
                });
            }
        }
    };

    rsx! {
        div {
            class: "{css::conv_messages}",
            "data-scroll-messages": "",
            onscroll: onscroll,
            if is_loading {
                div {
                    style: "text-align:center;padding:8px;color:var(--text-secondary);font-size:12px;",
                    "加载中..."
                }
            }
            if has_older {
                div {
                    style: "text-align:center;padding:8px;color:var(--text-secondary);font-size:12px;",
                    "↑ 向上滚动加载更多"
                }
            }
            for msg in &messages {
                MessageBubble { message: msg.clone(), streaming_reasoning: None }
            }
            {
                if show_streaming {
                    rsx! {
                        div {
                            class: "{css::conv_message_canvas}",
                            MessageBubble {
                                message: crate::Message {
                                    id: "streaming".into(),
                                    role: MessageRole::Assistant,
                                    content: stream_content,
                                    reasoning_content: None,
                                    timestamp: chrono::Utc::now(),
                                    status: MessageStatus::Sending,
                                },
                                streaming_reasoning: if stream_reasoning.is_empty() { None } else { Some(stream_reasoning) },
                            }
                            div {
                                class: "{css::conv_streaming_indicator}",
                                span {
                                    class: "{css::conv_streaming_cursor} animate-blink",
                                    "\u{258C}"
                                }
                            }
                        }
                    }
                } else {
                    rsx! { {} }
                }
            }
            if has_newer {
                div {
                    style: "text-align:center;padding:8px;color:var(--text-secondary);font-size:12px;",
                    "↓ 向下滚动加载更多"
                }
            }
        }
    }
}
