//! HTTP 请求重试共享逻辑。
//!
//! 4 个 AI Provider（OpenAI 兼容、Ollama、DeepSeek、OpenAI Responses）
//! 共用同一套重试策略，在此模块统一实现：
//!
//! - **可重试状态码**：429（限流）或 5xx（服务端错误）
//! - **可重试错误**：连接超时或网络连接错误
//! - **退避策略**：500ms × 2^(attempt-1) 指数退避
//!
//! 各 Provider 的 `send_with_retry` 通过 [`should_retry_result`] 统一判断
//! `Ok` / `Err` 两种情况是否需要重试，将原本 `match` + 双 `if` 分支
//! 合并为单一 `if`，降低认知复杂度。

use std::time::Duration;

/// 判断 HTTP 状态码是否应触发重试（429 或 5xx）。
#[inline]
pub fn should_retry_status(status: reqwest::StatusCode) -> bool {
    let code = status.as_u16();
    code == 429 || code >= 500
}

/// 判断 reqwest 错误是否应触发重试（超时或连接错误）。
#[inline]
pub fn should_retry_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

/// 判断成功的 HTTP 响应是否应触发重试（可重试状态码且未超过最大重试次数）。
#[inline]
pub fn should_retry_ok_response(
    status: reqwest::StatusCode,
    attempt: u32,
    max_retries: u32,
) -> bool {
    should_retry_status(status) && attempt < max_retries
}

/// 判断请求错误是否应触发重试（可重试错误且未超过最大重试次数）。
#[inline]
pub fn should_retry_err_response(
    error: &reqwest::Error,
    attempt: u32,
    max_retries: u32,
) -> bool {
    should_retry_error(error) && attempt < max_retries
}

/// 统一判断请求结果（`Ok` 或 `Err`）是否应触发重试。
///
/// 将 `send_with_retry` 中的 `match` + 双 `if` 分支合并为单一判断，
/// 降低主函数的认知复杂度（CC 6 → 3）。
///
/// # Arguments
///
/// * `result` - HTTP 请求结果（成功返回 `Response`，失败返回 `Error`）
/// * `attempt` - 当前尝试次数（从 1 开始）
/// * `max_retries` - 最大重试次数
#[inline]
pub fn should_retry_result(
    result: &Result<reqwest::Response, reqwest::Error>,
    attempt: u32,
    max_retries: u32,
) -> bool {
    match result {
        Ok(resp) => should_retry_ok_response(resp.status(), attempt, max_retries),
        Err(e) => should_retry_err_response(e, attempt, max_retries),
    }
}

/// 计算指数退避延迟：500ms * 2^(attempt-1)。
#[inline]
pub fn compute_backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(500 * 2u64.pow(attempt - 1))
}
