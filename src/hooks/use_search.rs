//! SearchStore Context 访问 Hook。
//!
//! 提供 `use_search()` 和 `use_search_provider()` 两个函数，
//! 分别用于获取已注入的 SearchStore 实例和初始化 Provider。
//! 本模块属于 hooks 层，桥接 `crate::stores::SearchStore` 与 Dioxus Context。

use dioxus::prelude::*;
use crate::stores::search::SearchStore;

/// 从 Dioxus Context 获取当前 [`SearchStore`] 实例。
///
/// 必须在 `use_search_provider()` 的子组件中调用，否则返回未初始化的 Store。
pub fn use_search() -> SearchStore {
    use_context::<SearchStore>()
}

/// 初始化 [`SearchStore`] Provider 并注入 Context。
///
/// 应在页面顶层组件中调用（如 `App` 组件），
/// 通过 `use_context_provider` 创建 Store 并注册到 Context 树。
pub fn use_search_provider() {
    use_context_provider(SearchStore::new);
}
