//! XEChat — 一款基于 Dioxus 的桌面 AI 聊天客户端
//!
//! 采用五层架构：views（视图） → components（组件） → hooks（钩子） → stores（状态） → services（服务）

// 国际化初始化
rust_i18n::i18n!("locales", fallback = "en");

/// SVG 图标组件（dioxus-iconify 自动生成）
pub mod icons;
/// 应用根组件
pub mod app;
/// 全局状态类型定义（ThemeMode、AppState、Toast 等）
pub mod state;
/// 可复用 UI 组件（sidebar、main_content、modals、notification）
pub mod components;
/// 视图层（页面级组件：布局、对话视图）
pub mod views;
/// 业务逻辑服务层（API 调用、文件读写、迁移等）
pub mod services;
/// 资产嵌入（logo 图片等）
pub mod assets;
/// 数据模型定义（Conversation、Message、Config 等）
pub mod models;
/// 工具函数（路径、Markdown 渲染、HTML 转义）
pub mod utils;
/// 全局钩子（use_app_provider 等自定义 hooks）
pub mod hooks;
/// 全局状态存储（Signal 驱动的响应式 Store）
pub mod stores;
/// 跨平台系统 API 封装（主题检测、语言检测等）
pub mod platform;

pub use models::*;
