// use dioxus::prelude::*;
// use crate::{Conversation, XEChatConfig};

/// 主题模式
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ThemeMode {
    /// 跟随系统主题
    System,
    /// 深色模式
    Dark,
    /// 浅色模式
    Light,
}

/// MainContent 区域的路由状态，互斥表示当前显示的页面。
#[derive(Clone, PartialEq, Debug)]
#[derive(Default)]
pub enum MainRoute {
    /// 欢迎页（无对话时默认显示）
    #[default]
    Welcome,
    /// 对话界面（包含对话 ID）
    Conversation(String),
    /// 设置页面
    Settings,
    /// 搜索页面
    Search,
}


#[derive(Clone, PartialEq)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub duration_ms: u64,
}

/// Toast 通知类型
#[derive(Clone, PartialEq)]
pub enum ToastKind {
    /// 信息提示
    Info,
    /// 成功提示
    Success,
    /// 错误提示
    Error,
}
