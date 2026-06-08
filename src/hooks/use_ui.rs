//! UIStore Context 访问 Hook。
//!
//! 提供 `use_ui()` 和 `use_ui_provider()` 两个函数，
//! 分别用于获取已注入的 UIStore 实例和初始化 Provider。
//! 本模块属于 hooks 层，桥接 `crate::stores::UIStore` 与 Dioxus Context。

use dioxus::prelude::*;
use crate::stores::UIStore;

/// 从 Dioxus Context 获取当前 [`UIStore`] 实例。
///
/// 必须在 `use_ui_provider()` 的子组件中调用，否则返回未初始化的 Store。
///
/// # Returns
///
/// 当前 Context 中的 [`UIStore`] 实例
pub fn use_ui() -> UIStore {
    use_context::<UIStore>()
}

/// 初始化 [`UIStore`] Provider 并注入 Context。
///
/// 应在页面顶层组件中调用（如 `Chat` 页面），
/// 通过 `use_context_provider` 创建 Store 并注册到 Context 树。
///
/// # Returns
///
/// 新创建并注入 Context 的 [`UIStore`] 实例
pub fn use_ui_provider() -> UIStore {
    use_context_provider(UIStore::new)
}
