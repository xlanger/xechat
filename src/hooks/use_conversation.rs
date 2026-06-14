//! ConversationStore Context 访问 Hook。
//!
//! 提供 `use_conversation()` 和 `use_conversation_provider()` 两个函数，
//! 分别用于获取已注入的 ConversationStore 实例和初始化 Provider 并加载对话数据。
//! 本模块属于 hooks 层，桥接 `crate::stores::ConversationStore` 与 Dioxus Context。

use dioxus::prelude::*;
use rust_i18n::t;
use crate::stores::ConversationStore;
use crate::stores::ui::ToastKind;

/// 从 Dioxus Context 获取当前 [`ConversationStore`] 实例。
///
/// 必须在 `use_conversation_provider()` 的子组件中调用，否则返回未初始化的 Store。
///
/// # Returns
///
/// 当前 Context 中的 [`ConversationStore`] 实例
pub fn use_conversation() -> ConversationStore {
    use_context::<ConversationStore>()
}

/// 初始化 [`ConversationStore`] Provider 并注入 Context，同时异步加载对话列表。
///
/// 启动时仅加载对话索引（id + title），不加载消息内容。
/// 点击 sidebar 对话项时，再按需加载该对话的完整内容。
///
/// # Returns
///
/// 新创建并注入 Context 的 [`ConversationStore`] 实例
pub fn use_conversation_provider() -> ConversationStore {
    let store = use_context_provider(ConversationStore::new);

    let provider_store = store.clone();
    use_effect(move || {
        let mut provider_store = provider_store.clone();
        spawn(async move {
            provider_store.init_backend().await;

            // 维度变更导致 turns 表重建时，显示 toast 提醒
            if provider_store.turns_rebuilt.read().clone() {
                let mut ui_store = crate::hooks::use_ui();
                let msg = t!("toast.turns-rebuilt").to_string();
                ui_store.show_toast(ToastKind::Info, msg, 5000);
                provider_store.turns_rebuilt.set(false);
            }
        });
    });

    store
}
