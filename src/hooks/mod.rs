//! XEChat Hooks 层。
//!
//! 本模块封装了 Dioxus Context 的访问 Hook，为页面组件提供 Store 实例的获取方式。
//! 每个子模块对应一个 Store，提供两个标准函数：
//!
//! - `use_*()` — 从 Context 获取已注入的 Store 实例
//! - `use_*_provider()` — 初始化 Store 并注入 Context（应在页面顶层调用）
//!
//! | 子模块 | 对应 Store | 用途 |
//! |--------|-----------|------|
//! | `use_app` | `AppStore` | 应用配置、主题、国际化 |
//! | `use_conversation` | `ConversationStore` | 对话列表、消息收发 |
//! | `use_ui` | `UIStore` | 弹窗、Toast、菜单状态 |

pub mod use_conversation;
pub mod use_ui;
pub mod use_app;
pub mod use_search;

pub use use_conversation::*;
pub use use_ui::*;
pub use use_app::*;
pub use use_search::*;
