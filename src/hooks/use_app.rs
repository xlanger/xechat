//! AppStore Context 访问 Hook。
//!
//! 提供 `use_app()` 和 `use_app_provider()` 两个函数，
//! 分别用于获取已注入的 AppStore 实例和初始化 Provider 并加载数据。
//! 本模块属于 hooks 层，桥接 `crate::stores::AppStore` 与 Dioxus Context。

use dioxus::prelude::*;
use crate::stores::AppStore;

/// 从 Dioxus Context 获取当前 [`AppStore`] 实例。
///
/// 必须在 `use_app_provider()` 的子组件中调用，否则返回未初始化的 Store。
///
/// # Returns
///
/// 当前 Context 中的 [`AppStore`] 实例
pub fn use_app() -> AppStore {
    use_context::<AppStore>()
}

/// 初始化 [`AppStore`] Provider 并注入 Context，同时异步加载应用配置。
///
/// 应在页面顶层组件中调用（如 `Conversation` 页面），
/// 通过 `use_context_provider` 创建 Store 并注册到 Context 树，
/// 然后在 `use_effect` 中触发配置加载。
///
/// # Returns
///
/// 新创建并注入 Context 的 [`AppStore`] 实例
pub fn use_app_provider() -> AppStore {
    let store = use_context_provider(AppStore::new);

    let provider_store = store;
    use_effect(move || {
        let mut provider_store = provider_store;
        spawn(async move {
            provider_store.load_config().await;
        });
    });

    store
}
