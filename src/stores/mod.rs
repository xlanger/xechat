//! XEChat 存储层（Stores）。
//!
//! 本模块封装了应用的核心响应式状态管理，分为三个子模块：
//!
//! - `app` — 应用全局状态（配置、主题、国际化）
//! - `conversation` — 聊天业务状态（对话列表、流式输出、消息收发）
//! - `ui` — UI 交互状态（弹窗、Toast 通知、右键菜单）
//!
//! 所有 Store 均基于 Dioxus 的 `Signal` 实现响应式数据绑定，
//! 通过 hooks 层注入到组件树中供页面和组件使用。

pub mod conversation;
pub mod ui;
pub mod app;
pub mod search;

pub use conversation::ConversationStore;
pub use conversation::StreamAction;
pub use conversation::StreamLoopAction;
pub use conversation::compute_full_window_range;
pub use conversation::compute_anchored_window_range;
pub use conversation::can_load_older;
pub use conversation::can_load_newer;
pub use conversation::compute_older_window;
pub use conversation::compute_newer_window_end;
pub use conversation::sync_ollama_host_to_provider;
pub use conversation::should_enable_ollama;
pub use ui::{UIStore, Toast, ToastKind};
pub use app::AppStore;
pub use search::SearchStore;
