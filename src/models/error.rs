//! 应用标准错误类型。
//!
//! 定义 [`AppError`] 枚举，覆盖网络、认证、API、流式、配置、IO、序列化、输入校验等所有错误域。
//! 每个变体携带结构化上下文数据，通过 [`AppError::i18n_key()`] 方法映射到 i18n 翻译键，
//! 实现用户提示的本地化。本模块属于 models 层，零 UI/I/O 依赖。

use std::fmt;
use serde_json;
use toml;

/// 应用统一错误类型。
///
/// 覆盖所有业务域的错误场景，每个变体携带足够的结构化上下文信息，
/// 用于日志记录、调试追踪和通过 [`AppError::i18n_key()`] 生成用户友好的本地化提示。
///
/// # 错误域划分
///
/// | 域 | 变体 | 典型场景 |
/// |----|------|---------|
/// | 网络 | [`AppError::Network`] | 连接超时、DNS 解析失败 |
/// | 认证 | [`AppError::Auth`] | API Key 无效、401 响应 |
/// | API | [`AppError::Api`] | 非 2xx 状态码 + 服务端错误体 |
/// | 流式 | [`AppError::Stream`] | SSE 流读取异常 |
/// | 配置 | [`AppError::Config`] | 配置文件读写/TOML 解析/环境变量 |
/// | IO | [`AppError::Io`] | 文件系统操作（创建目录、读写文件） |
/// | 序列化 | [`AppError::Serialization`] | JSON/TOML 序列化反序列化 |
/// | 输入校验 | [`AppError::InvalidInput`] | 字段验证失败 |
/// | 不支持 | [`AppError::Unsupported`] | 未实现的协议/操作 |
///
/// # Example
///
/// ```ignore
/// use crate::models::error::{AppError, AuthFailReason};
///
/// // 构造认证错误
/// let err = AppError::Auth {
///     reason: AuthFailReason::InvalidKeyFormat,
/// };
///
/// // 获取 i18n 翻译键和参数
/// let (key, args) = err.i18n_key();
/// assert_eq!(key, "error.auth.invalidKey");
/// ```
#[derive(Debug, Clone)]
pub enum AppError {
    /// 网络层故障（连接超时、DNS 解析失败、TLS 握手错误等）。
    Network {
        /// 原始错误描述（来自底层库的错误信息）
        detail: String,
    },

    /// API 认证失败。
    Auth {
        /// 具体的认证失败原因
        reason: AuthFailReason,
    },

    /// 服务端返回非成功 HTTP 状态码。
    Api {
        /// HTTP 状态码（如 400、403、429、500 等）
        status: u16,
        /// 服务端响应体中的错误消息（已尝试提取可读部分）
        body: Option<String>,
    },

    /// SSE 流式响应读取异常。
    Stream {
        /// 原始错误描述
        detail: String,
    },

    /// 配置文件相关错误（读取、写入、解析、环境变量）。
    Config {
        /// 操作类型标识（`"read"` / `"write"` / `"parse"` / `"env"`）
        operation: String,
        /// 附加上下文或原始错误信息
        detail: String,
    },

    /// 文件系统 I/O 错误。
    Io {
        /// 执行的操作描述（如 `"create directory"`、`"write file"`）
        operation: String,
        /// 原始错误信息
        detail: String,
    },

    /// 数据序列化/反序列化错误。
    Serialization {
        /// 数据格式（`"json"` 或 `"toml"`）
        format: String,
        /// 原始错误信息
        detail: String,
    },

    /// 输入校验失败。
    InvalidInput {
        /// 校验未通过的字段名
        field: String,
        /// 校验失败原因
        reason: String,
    },

    /// 不支持的协议或操作。
    Unsupported {
        /// 不支持的项名称（如协议标识符）
        item: String,
    },
}

/// API 认证失败的具体原因分类。
#[derive(Debug, Clone)]
pub enum AuthFailReason {
    /// API Key 格式无效（无法构造合法的 HTTP Bearer token header 值）。
    InvalidKeyFormat,

    /// 服务端返回 401 Unauthorized。
    Unauthorized {
        /// 配置文件路径（用于提示用户检查位置）
        config_path: String,
    },
}

// ── Display 辅助函数（按错误类别分组，每组 ≤5 arms） ──────────

/// 格式化基础设施类错误（Network / Stream / Unsupported）。
fn fmt_infra_error(err: &AppError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match err {
        AppError::Network { detail } => write!(f, "Network error: {}", detail),
        AppError::Stream { detail } => write!(f, "Stream error: {}", detail),
        AppError::Unsupported { item } => write!(f, "Unsupported: {}", item),
        _ => unreachable!(),
    }
}

/// 格式化操作类错误（Config / Io / Serialization）。
fn fmt_op_error(err: &AppError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match err {
        AppError::Config { operation, detail } => write!(f, "Config error ({}): {}", operation, detail),
        AppError::Io { operation, detail } => write!(f, "IO error ({}): {}", operation, detail),
        AppError::Serialization { format, detail } => write!(f, "Serialization error ({}): {}", format, detail),
        _ => unreachable!(),
    }
}

/// 格式化业务类错误（Auth / Api / InvalidInput）。
fn fmt_business_error(err: &AppError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match err {
        AppError::Auth { reason } => fmt_auth_reason(reason, f),
        AppError::Api { status, body } => fmt_api_error(status, body, f),
        AppError::InvalidInput { field, reason } => write!(f, "Invalid input '{}': {}", field, reason),
        _ => unreachable!(),
    }
}

/// 错误类别，用于将 9-arm match 拆分为 3 个 ≤4-arm 的子 match。
#[derive(Clone, Copy)]
enum ErrorCategory {
    Infra,
    Op,
    Business,
}

impl AppError {
    /// 返回错误所属类别。
    fn category(&self) -> ErrorCategory {
        match self {
            Self::Network { .. } | Self::Stream { .. } | Self::Unsupported { .. } => ErrorCategory::Infra,
            Self::Config { .. } | Self::Io { .. } | Self::Serialization { .. } => ErrorCategory::Op,
            Self::Auth { .. } | Self::Api { .. } | Self::InvalidInput { .. } => ErrorCategory::Business,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.category() {
            ErrorCategory::Infra => fmt_infra_error(self, f),
            ErrorCategory::Op => fmt_op_error(self, f),
            ErrorCategory::Business => fmt_business_error(self, f),
        }
    }
}

fn fmt_auth_reason(reason: &AuthFailReason, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match reason {
        AuthFailReason::InvalidKeyFormat => write!(f, "Authentication error: invalid API key format"),
        AuthFailReason::Unauthorized { .. } => write!(f, "Authentication error: unauthorized (401)"),
    }
}

fn fmt_api_error(status: &u16, body: &Option<String>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(b) = body {
        write!(f, "API error ({}): {}", status, b)
    } else {
        write!(f, "API error ({})", status)
    }
}

impl std::error::Error for AppError {}

// ── i18n 辅助函数（按错误类别分组，每组 ≤5 arms） ──────────

/// 基础设施类错误的 i18n 键值对。
fn infra_i18n_key(err: &AppError) -> (&str, Vec<(String, String)>) {
    match err {
        AppError::Network { detail } => ("error.network", vec![("detail".into(), detail.clone())]),
        AppError::Stream { detail } => ("error.stream.readError", vec![("detail".into(), detail.clone())]),
        AppError::Unsupported { item } => ("error.unsupported", vec![("item".into(), item.clone())]),
        _ => unreachable!(),
    }
}

/// 操作类错误的 i18n 键值对。
fn op_i18n_key(err: &AppError) -> (&str, Vec<(String, String)>) {
    match err {
        AppError::Config { operation, detail } => (
            "error.config.failed",
            vec![("operation".into(), operation.clone()), ("detail".into(), detail.clone())],
        ),
        AppError::Io { operation, detail } => (
            "error.io.failed",
            vec![("operation".into(), operation.clone()), ("detail".into(), detail.clone())],
        ),
        AppError::Serialization { format, detail } => (
            "error.serialization.parseError",
            vec![("format".into(), format.clone()), ("detail".into(), detail.clone())],
        ),
        _ => unreachable!(),
    }
}

/// 业务类错误的 i18n 键值对。
fn business_i18n_key(err: &AppError) -> (&str, Vec<(String, String)>) {
    match err {
        AppError::Auth { reason } => auth_i18n_key(reason),
        AppError::Api { status, body } => api_i18n_key(status, body),
        AppError::InvalidInput { field, reason } => (
            "error.invalidInput",
            vec![("field".into(), field.clone()), ("reason".into(), reason.clone())],
        ),
        _ => unreachable!(),
    }
}

impl AppError {
    /// 返回此错误的 i18n 翻译键和模板参数。
    ///
    /// stores 层调用此方法获取翻译键和参数后，
    /// 通过 `app_store.tf(key, &args)` 生成本地化的用户提示文本。
    ///
    /// # Returns
    ///
    /// 元组 `(i18n_key, replacements)`：
    /// - `i18n_key`: 点分隔的翻译键（如 `"error.network"`）
    /// - `replacements`: 模板参数列表，每个元素为 `(占位符名, 替换值)`
    ///
    /// # Example
    ///
    /// ```ignore
    /// let err = AppError::Network { detail: "connection timed out".into() };
    /// let (key, args) = err.i18n_key();
    /// // key = "error.network", args = [("detail", "connection timed out")]
    /// let msg = app_store.tf(key, &args);
    /// ```
    pub fn i18n_key(&self) -> (&str, Vec<(String, String)>) {
        match self.category() {
            ErrorCategory::Infra => infra_i18n_key(self),
            ErrorCategory::Op => op_i18n_key(self),
            ErrorCategory::Business => business_i18n_key(self),
        }
    }
}

fn auth_i18n_key(reason: &AuthFailReason) -> (&str, Vec<(String, String)>) {
    match reason {
        AuthFailReason::InvalidKeyFormat => ("error.auth.invalidKey", vec![]),
        AuthFailReason::Unauthorized { config_path } => {
            ("error.auth.unauthorized", vec![("path".into(), config_path.clone())])
        }
    }
}

fn api_i18n_key(status: &u16, body: &Option<String>) -> (&'static str, Vec<(String, String)>) {
    let mut args = vec![("status".into(), status.to_string())];
    if let Some(b) = body {
        args.push(("body".into(), b.clone()));
    }
    ("error.api.httpError", args)
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network {
            detail: err.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Io {
            operation: "fs".into(),
            detail: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            format: "json".into(),
            detail: err.to_string(),
        }
    }
}

impl From<toml::de::Error> for AppError {
    fn from(err: toml::de::Error) -> Self {
        Self::Config {
            operation: "parse".into(),
            detail: err.to_string(),
        }
    }
}

impl From<toml::ser::Error> for AppError {
    fn from(err: toml::ser::Error) -> Self {
        Self::Config {
            operation: "serialize".into(),
            detail: err.to_string(),
        }
    }
}
