//! 嵌入模型下载服务。
//!
//! 从 GitHub Release 下载 E5 GGUF 嵌入模型文件，支持多源降级和进度回调。
//! 下载源优先级：GitHub Release → hf-mirror → HuggingFace。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

/// 模型文件名。
const MODEL_FILENAME: &str = "multilingual-e5-base-q8_0.gguf";

/// 预期模型文件大小（约 430MB，Q8_0 量化）。
const EXPECTED_MODEL_SIZE: u64 = 430_000_000;

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
        url: "https://github.com/xlanger/xechat/releases/download/models/multilingual-e5-base-q8_0.gguf",
    },
    DownloadSource {
        name: "hf-mirror",
        url: "https://hf-mirror.com/ggml-org/multilingual-e5-base-GGUF/resolve/main/multilingual-e5-base-q8_0.gguf",
    },
    DownloadSource {
        name: "huggingface",
        url: "https://huggingface.co/ggml-org/multilingual-e5-base-GGUF/resolve/main/multilingual-e5-base-q8_0.gguf",
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
        eprintln!("[model-downloader] Trying source: {}", source.name);

        match download_from_source(&source.url, &temp_path, &target_path, &on_progress).await {
            Ok(()) => {
                on_progress(DownloadProgress::Completed);
                eprintln!("[model-downloader] Download completed from {}", source.name);
                return Ok(target_path);
            }
            Err(e) => {
                eprintln!("[model-downloader] Source {} failed: {}", source.name, e);
                last_error = Some(e);
                // 清理临时文件
                let _ = std::fs::remove_file(&temp_path);
            }
        }
    }

    let err_msg = last_error.map(|e| e.to_string()).unwrap_or_else(|| "All sources failed".to_string());
    on_progress(DownloadProgress::Failed(err_msg.clone()));
    Err(anyhow::anyhow!("All download sources failed. Last error: {}", err_msg))
}

/// 从单个源下载文件，支持断点续传。
async fn download_from_source(
    url: &str,
    temp_path: &Path,
    target_path: &Path,
    on_progress: &Arc<dyn Fn(DownloadProgress) + Send + Sync>,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;

    // 检查是否有未完成的临时文件（断点续传）
    let mut start_from: u64 = 0;
    if temp_path.exists() {
        if let Ok(meta) = temp_path.metadata() {
            start_from = meta.len();
            eprintln!("[model-downloader] Resuming from {} bytes", start_from);
        }
    }

    let mut request = client.get(url);
    if start_from > 0 {
        request = request.header("Range", format!("bytes={}-", start_from));
    }

    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() && status.as_u16() != 206 {
        anyhow::bail!("HTTP error: {}", status);
    }

    let total_size = if status.as_u16() == 206 {
        // Partial Content - 读取 Content-Range 获取总大小
        response.headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    } else {
        response.content_length().unwrap_or(0)
    };

    // 打开临时文件（追加模式）
    let mut file = if start_from > 0 {
        std::fs::OpenOptions::new().append(true).open(temp_path)?
    } else {
        std::fs::File::create(temp_path)?
    };

    use std::io::Write;
    use futures_util::StreamExt;

    let mut downloaded = start_from;
    let mut stream = response.bytes_stream();
    let mut last_report = std::time::Instant::now();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;

        // 每 500ms 报告一次进度
        if last_report.elapsed() > std::time::Duration::from_millis(500) {
            on_progress(DownloadProgress::Downloading(downloaded, total_size));
            last_report = std::time::Instant::now();
        }
    }

    // 最终进度报告
    on_progress(DownloadProgress::Downloading(downloaded, total_size));

    // 校验文件大小
    if downloaded < EXPECTED_MODEL_SIZE / 2 {
        anyhow::bail!("Downloaded file too small: {} bytes (expected ~{} bytes)", downloaded, EXPECTED_MODEL_SIZE);
    }

    // 复制临时文件到最终路径（跨文件系统安全），然后删除临时文件
    std::fs::copy(temp_path, target_path)
        .with_context(|| "Failed to copy temp file to target")?;
    let _ = std::fs::remove_file(temp_path);

    Ok(())
}
