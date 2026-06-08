//! UI 状态管理 Store。
//!
//! 持有弹窗显示状态、Toast 通知、菜单开关等纯 UI 交互状态。
//! 本模块属于 stores 层，不依赖任何 I/O 服务。

use dioxus::prelude::*;

/// Toast 通知类型枚举。
///
/// 用于区分不同语义的提示消息，决定 UI 展示样式和图标。
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ToastKind {
    /// 信息提示（蓝色）
    Info,
    /// 成功提示（绿色）
    Success,
    /// 错误提示（红色）
    Error,
}

/// Toast 通知数据结构，包含消息内容、类型和持续时间。
#[derive(Clone, PartialEq)]
pub struct Toast {
    /// 通知文本内容
    pub message: String,
    /// 通知类型（Info / Success / Error）
    pub kind: ToastKind,
    /// 显示时长（毫秒）
    pub duration_ms: u64,
}

/// UI 状态 Store，管理弹窗、Toast 和菜单等界面交互状态。
///
/// 通过信号控制各 UI 组件的显隐，提供 [`UIStore::show_toast()`] / [`UIStore::hide_toast()`] 方法操作通知。
#[derive(Copy, Clone)]
pub struct UIStore {
    /// 是否显示设置弹窗
    pub show_config_modal: Signal<bool>,
    /// 重命名弹窗的目标对话 ID（`None` 表示隐藏）
    pub show_rename_modal: Signal<Option<String>>,
    /// 删除确认弹窗的目标对话 ID（`None` 表示隐藏）
    pub show_delete_modal: Signal<Option<String>>,
    /// 当前打开的右键菜单对应的元素 ID（`None` 表示无菜单打开）
    pub open_menu_id: Signal<Option<String>>,
    /// 是否打开顶部栏下拉菜单
    pub open_header_menu: Signal<bool>,
    /// 当前活跃的 Toast 通知（`None` 表示无通知）
    pub active_toast: Signal<Option<Toast>>,
    /// 右键菜单位置坐标 (x, y)（`None` 表示无定位）
    pub menu_position: Signal<Option<(f64, f64)>>,
}

impl Default for UIStore {
    fn default() -> Self {
        Self::new()
    }
}

impl UIStore {
    /// 创建 UIStore 实例并初始化所有信号为默认值。
    ///
    /// 默认值：
    /// - `show_rename_modal`: `None`
    /// - `show_delete_modal`: `None`
    /// - `open_menu_id`: `None`
    /// - `open_header_menu`: `false`
    /// - `active_toast`: `None`
    /// - `menu_position`: `None`
    pub fn new() -> Self {
        Self {
            show_config_modal: Signal::new(false),
            show_rename_modal: Signal::new(None),
            show_delete_modal: Signal::new(None),
            open_menu_id: Signal::new(None),
            open_header_menu: Signal::new(false),
            active_toast: Signal::new(None),
            menu_position: Signal::new(None),
        }
    }

    /// 显示指定类型和内容的 Toast 通知。
    ///
    /// # Arguments
    ///
    /// * `kind` - 通知类型（[`ToastKind::Info`] / [`ToastKind::Success`] / [`ToastKind::Error`]）
    /// * `message` - 通知文本内容
    /// * `duration_ms` - 显示持续时长（毫秒）
    pub fn show_toast(&mut self, kind: ToastKind, message: String, duration_ms: u64) {
        self.active_toast.set(Some(Toast {
            message,
            kind,
            duration_ms,
        }));
    }

    /// 隐藏当前活跃的 Toast 通知。
    pub fn hide_toast(&mut self) {
        self.active_toast.set(None);
    }
}
