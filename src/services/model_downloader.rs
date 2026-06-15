//! 嵌入模型下载服务。
//!
//! 从 GitHub Release 下载 Qwen3-Embedding-0.6B GGUF 嵌入模型文件，支持多源降级和进度回调。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

/// 模型文件名。
const MODEL_FILENAME: &str = "qwen3-embedding-0.6b-q8_0.gguf";

/// 预期模型文件大小（约 700MB，Q8_0 量化）。
const EXPECTED_MODEL_SIZE: u64 = 700_000_000;

/// 下载源定义。
struct DownloadSource {
    /// 源名称（用于日志）。
    name: &'static str,
    /// 下载 URL。
    url: &'static str,
}

/// 下载源列表（按优先级排序）。
const DOWNLOAD_SOURCES: &[DownloadSource] = &[
    DownloadSource {
        name: "github",
        url: "https://github.com/xlanger/xechat/releases/download/models/qwen3-embedding-0.6b-q8_0.gguf",
    },
    DownloadSource {
        name: "ghproxy",
        url: "https://ghfast.top/https://github.com/xlanger/xechat/releases/download/models/qwen3-embedding-0.6b-q8_0.gguf",
    },
];

/// 模型下载进度。
#[derive(Clone, Debug)]
pub enum DownloadProgress {
    /// 正在下载，参数为 (已下载字节数, 总字节数)。
    Downloading(u64, u64),
    /// 下载完成。
    Completed,
    /// 下载失败，参数为错误信息。
    Failed(String),
}

/// 获取模型存储目录（跨平台标准数据路径）。
///
/// - macOS: `~/Library/Application Support/XEChat/models/`
/// - Linux: `~/.local/share/XEChat/models/`
/// - Windows: `%LOCALAPPDATA%\XEChat\models\`
pub fn get_models_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("XEChat")
        .join("models")
}

/// 获取模型文件完整路径。
pub fn get_model_path() -> PathBuf {
    get_models_dir().join(MODEL_FILENAME)
}

/// 检查模型文件是否已存在且大小合理。
pub fn is_model_ready() -> bool {
    let path = get_model_path();
    path.exists() && path.metadata().map(|m| m.len() > EXPECTED_MODEL_SIZE / 2).unwrap_or(false)
}

/// 尝试从单个源下载，成功时回调进度并返回路径。
///
/// 失败时清理临时文件并返回错误。
async fn try_download_source(
    source: &DownloadSource,
    temp_path: &Path,
    target_path: &Path,
    on_progress: &Arc<dyn Fn(DownloadProgress) + Send + Sync>,
) -> Result<PathBuf> {
    eprintln!("[model-downloader] Trying source: {}", source.name);

    match download_from_source(&source.url, temp_path, target_path, on_progress).await {
        Ok(()) => {
            on_progress(DownloadProgress::Completed);
            eprintln!("[model-downloader] Download completed from {}", source.name);
            Ok(target_path.to_path_buf())
        }
        Err(e) => {
            eprintln!("[model-downloader] Source {} failed: {}", source.name, e);
            // 清理临时文件
            let _ = std::fs::remove_file(temp_path);
            Err(e)
        }
    }
}

/// 下载模型文件。
///
/// 按优先级依次尝试下载源，任一成功即停止。
/// 下载过程中通过 `on_progress` 回调报告进度。
///
/// # Arguments
///
/// * `on_progress` - 进度回调函数，参数为 `DownloadProgress`
///
/// # Errors
///
/// 所有下载源均失败时返回错误。
pub async fn download_model(on_progress: Arc<dyn Fn(DownloadProgress) + Send + Sync>) -> Result<PathBuf> {
    let models_dir = get_models_dir();
    std::fs::create_dir_all(&models_dir)
        .with_context(|| format!("Failed to create models dir: {}", models_dir.display()))?;

    let target_path = models_dir.join(MODEL_FILENAME);
    let temp_path = models_dir.join(format!("{MODEL_FILENAME}.downloading"));

    let mut last_error = None;

    for source in DOWNLOAD_SOURCES {
        match try_download_source(source, &temp_path, &target_path, &on_progress).await {
            Ok(path) => return Ok(path),
            Err(e) => { last_error = Some(e); }
        }
    }

    let err_msg = last_error.map(|e| e.to_string()).unwrap_or_else(|| "All sources failed".to_string());
    on_progress(DownloadProgress::Failed(err_msg.clone()));
    Err(anyhow::anyhow!("All download sources failed. Last error: {}", err_msg))
}

/// 创建带超时配置的 HTTP 客户端。
pub fn create_download_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")
}

/// 检查临时文件是否存在，返回断点续传的起始字节位置。
pub fn get_resume_position(temp_path: &Path) -> u64 {
    if temp_path.exists() {
        if let Ok(meta) = temp_path.metadata() {
            let pos = meta.len();
            eprintln!("[model-downloader] Resuming from {} bytes", pos);
            return pos;
        }
    }
    0
}

/// 构建并发送下载请求，支持 Range 头断点续传。
async fn send_download_request(
    client: &reqwest::Client,
    url: &str,
    start_from: u64,
) -> Result<reqwest::Response> {
    let mut request = client.get(url);
    if start_from > 0 {
        request = request.header("Range", format!("bytes={}-", start_from));
    }
    request.send().await.with_context(|| format!("Failed to send download request to {}", url))
}

/// 从 Content-Range 响应头中解析文件总大小。
///
/// Content-Range 格式：`bytes start-end/total`，提取 `total` 部分。
///
/// # Arguments
///
/// * `response` - HTTP 响应引用
///
/// # Returns
///
/// 解析成功返回总大小，解析失败返回 0。
pub fn parse_content_range_total(response: &reqwest::Response) -> u64 {
    response.headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
}

/// 从 HTTP 响应中提取文件总大小，验证状态码。
///
/// 支持 200 OK（Content-Length）和 206 Partial Content（Content-Range）两种响应。
fn extract_total_size(response: &reqwest::Response) -> Result<u64> {
    let status = response.status();
    if !status.is_success() && status.as_u16() != 206 {
        anyhow::bail!("HTTP error: {}", status);
    }

    let total_size = if status.as_u16() == 206 {
        parse_content_range_total(response)
    } else {
        response.content_length().unwrap_or(0)
    };

    Ok(total_size)
}

/// 打开临时文件，断点续传时使用追加模式，否则创建新文件。
fn open_temp_file(temp_path: &Path, start_from: u64) -> Result<std::fs::File> {
    if start_from > 0 {
        std::fs::OpenOptions::new().append(true).open(temp_path)
            .with_context(|| format!("Failed to open temp file for append: {}", temp_path.display()))
    } else {
        std::fs::File::create(temp_path)
            .with_context(|| format!("Failed to create temp file: {}", temp_path.display()))
    }
}

/// 写入一个数据块到文件，并在超过 500ms 间隔时报告进度。
///
/// # Returns
///
/// 更新后的已下载字节数。
fn write_chunk_with_progress(
    file: &mut std::fs::File,
    chunk: &[u8],
    downloaded: u64,
    total_size: u64,
    last_report: &mut std::time::Instant,
    on_progress: &Arc<dyn Fn(DownloadProgress) + Send + Sync>,
) -> std::io::Result<u64> {
    use std::io::Write;
    file.write_all(chunk)?;
    let downloaded = downloaded + chunk.len() as u64;

    if last_report.elapsed() > std::time::Duration::from_millis(500) {
        on_progress(DownloadProgress::Downloading(downloaded, total_size));
        *last_report = std::time::Instant::now();
    }

    Ok(downloaded)
}

/// 完成下载：校验文件大小，复制临时文件到目标路径，删除临时文件。
fn finalize_download(downloaded: u64, temp_path: &Path, target_path: &Path) -> Result<()> {
    if downloaded < EXPECTED_MODEL_SIZE / 2 {
        anyhow::bail!(
            "Downloaded file too small: {} bytes (expected ~{} bytes)",
            downloaded,
            EXPECTED_MODEL_SIZE
        );
    }

    std::fs::copy(temp_path, target_path)
        .with_context(|| "Failed to copy temp file to target")?;
    let _ = std::fs::remove_file(temp_path);

    Ok(())
}

/// 初始化下载上下文：创建客户端、发送请求、提取文件大小、打开临时文件。
pub async fn init_download_context(
    url: &str,
    temp_path: &Path,
) -> Result<(reqwest::Response, u64, std::fs::File)> {
    let client = create_download_client()?;
    let start_from = get_resume_position(temp_path);
    let response = send_download_request(&client, url, start_from).await?;
    let total_size = extract_total_size(&response)?;
    let file = open_temp_file(temp_path, start_from)?;
    Ok((response, total_size, file))
}

/// 从 HTTP 响应流中读取数据块并写入文件，返回累计已下载字节数。
pub async fn stream_chunks_to_file(
    response: reqwest::Response,
    file: &mut std::fs::File,
    start_from: u64,
    total_size: u64,
    on_progress: &Arc<dyn Fn(DownloadProgress) + Send + Sync>,
) -> Result<u64> {
    use futures_util::StreamExt;

    let mut downloaded = start_from;
    let mut stream = response.bytes_stream();
    let mut last_report = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        downloaded = write_chunk_with_progress(
            file, &chunk, downloaded, total_size, &mut last_report, on_progress,
        )?;
    }

    // 最终进度报告
    on_progress(DownloadProgress::Downloading(downloaded, total_size));

    Ok(downloaded)
}

/// 从单个源下载文件，支持断点续传。
async fn download_from_source(
    url: &str,
    temp_path: &Path,
    target_path: &Path,
    on_progress: &Arc<dyn Fn(DownloadProgress) + Send + Sync>,
) -> Result<()> {
    let (response, total_size, mut file) = init_download_context(url, temp_path).await?;
    let downloaded = stream_chunks_to_file(response, &mut file, get_resume_position(temp_path), total_size, on_progress).await?;
    finalize_download(downloaded, temp_path, target_path)
}
