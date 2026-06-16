//! Qwen3-Embedding GGUF 嵌入器实现。
//!
//! 通过 embellama（封装 llama-cpp-2）加载 Qwen3-Embedding-0.6B GGUF 模型，
//! 使用 Qwen3 指令模板区分查询和段落编码。
//!
//! # 指令模板
//!
//! Qwen3-Embedding 要求输入文本带指令模板：
//! - 检索（query）：`<Instruct>:检索相关文章<Query>:查询文本`
//! - 存储（passage）：`<Instruct>:检索相关文章<Document>:文档文本`
//!
//! # 线程安全设计（单线程 Worker）
//!
//! embellama 底层使用 llama-cpp-2，其 `LlamaContext` 是 `!Send`，
//! 且模型存储在 **线程本地存储 (TLS)** 中。
//! 若每次 encode 都用 `spawn_blocking`，Tokio 阻塞线程池的 512 个线程
//! 会导致每次可能分配到不同线程 → TLS 为空 → 重新加载模型（~3s 开销）。
//!
//! 因此采用 **专用 Worker 线程 + mpsc channel** 模式：
//! - 构造时启动一个专用 std::thread，在其中加载模型并常驻内存
//! - 所有 encode 请求通过 channel 发送到该线程处理
//! - 模型永远存在于 Worker 的 TLS 中，零重复加载

use std::path::PathBuf;
use std::sync::Mutex;

use async_trait::async_trait;
use embellama::{EmbeddingEngine, EngineConfig, ModelConfig, NormalizationMode, PoolingStrategy};
use tokio::sync::{mpsc, oneshot};

use super::Embedder;

/// Qwen3-Embedding 指令前缀（检索任务）。
pub const INSTRUCT_PREFIX: &str = "检索相关文章";

/// 模型文件名。
pub const MODEL_FILENAME: &str = "qwen3-embedding-0.6b-q8_0.gguf";

/// Worker 线程可处理的请求类型。
enum EmbedRequest {
    /// 编码查询文本（带 Query 指令模板）。
    Query { text: String, response: oneshot::Sender<anyhow::Result<Vec<f32>>> },
    /// 编码段落文本（带 Document 指令模板）。
    Passage { text: String, response: oneshot::Sender<anyhow::Result<Vec<f32>>> },
    /// 批量编码（用于 encode 方法）。
    Batch { texts: Vec<String>, response: oneshot::Sender<anyhow::Result<Vec<Vec<f32>>>> },
}

/// 基于 Qwen3-Embedding-0.6B GGUF 的本地嵌入器。
///
/// 使用专用 Worker 线程持有 `EmbeddingEngine`，确保模型常驻于同一线程的 TLS 中，
/// 避免因 `LlamaContext` 的 `!Send` + TLS 设计导致的重复加载。
pub struct Qwen3Embedder {
    /// 请求发送端，所有编码请求通过此 channel 发给 Worker 线程。
    sender: mpsc::Sender<EmbedRequest>,
    dimension: usize,
    context_window: usize,
}

/// 解析模型文件路径。
///
/// 按以下顺序尝试查找：
/// 1. 标准数据目录：由模型下载服务下载到的位置（跨平台标准路径）
/// 2. 开发环境：当前工作目录 / `assets/models/`
/// 3. macOS 打包：`.app/Contents/Resources/assets/models/`
/// 4. 其他平台打包：可执行文件同级目录 / `assets/models/`
pub fn resolve_model_path() -> PathBuf {
    // 1. 标准数据目录（模型下载服务下载到的位置）
    let data_path = crate::services::model_downloader::get_model_path();
    if data_path.exists() {
        return data_path;
    }

    // 2. 开发环境
    let dev_path = std::env::current_dir()
        .unwrap_or_default()
        .join("assets")
        .join("models")
        .join(MODEL_FILENAME);
    if dev_path.exists() {
        return dev_path;
    }

    // 3. 从可执行文件路径推导（打包后）
    if let Some(bundled) = resolve_bundled_model_path(MODEL_FILENAME) {
        return bundled;
    }

    // 兜底：返回标准数据目录路径（不存在时会报错，提示用户下载模型）
    data_path
}

/// 从可执行文件路径推导打包后的模型路径。
///
/// macOS: `XEChat.app/Contents/MacOS/xechat` → `../Resources/assets/models/`
/// Windows/Linux: 与 exe 同级的 `assets/models/`
pub fn resolve_bundled_model_path(model_name: &str) -> Option<PathBuf> {
    let exe_path = std::env::current_exe().ok()?;
    let exe_dir = exe_path.parent()?;

    #[cfg(target_os = "macos")]
    {
        if let Some(path) = resolve_macos_resources_path(exe_dir, model_name) {
            return Some(path);
        }
    }

    resolve_platform_bundled_path(exe_dir, model_name)
}

/// 解析 macOS Resources 目录下的模型路径。
#[cfg(target_os = "macos")]
fn resolve_macos_resources_path(exe_dir: &std::path::Path, model_name: &str) -> Option<PathBuf> {
    let macos_resources = exe_dir
        .parent()
        .map(|p| p.join("Resources"))
        .unwrap_or_default()
        .join("assets")
        .join("models")
        .join(model_name);
    if macos_resources.exists() {
        Some(macos_resources)
    } else {
        None
    }
}

/// 解析通用平台打包路径（exe 同级目录）。
fn resolve_platform_bundled_path(exe_dir: &std::path::Path, model_name: &str) -> Option<PathBuf> {
    let bundled_path = exe_dir
        .join("assets")
        .join("models")
        .join(model_name);
    if bundled_path.exists() {
        Some(bundled_path)
    } else {
        None
    }
}

/// 构建 Qwen3-Embedding 模型配置。
fn build_model_config(model_path: &PathBuf) -> anyhow::Result<ModelConfig> {
    ModelConfig::builder()
        .with_model_path(model_path.to_str().unwrap_or_default())
        .with_model_name("qwen3-embedding")
        .with_normalization_mode(NormalizationMode::L2)
        .with_pooling_strategy(PoolingStrategy::Last)
        .build()
        .map_err(|e| anyhow::anyhow!("Build model config failed: {}", e))
}

/// 构建引擎配置并创建嵌入引擎。
fn create_engine(model_config: ModelConfig) -> anyhow::Result<(EmbeddingEngine, usize)> {
    // Decoder 模式默认 n_batch=2048, n_seq_max=2 → effective_max=1022
    // 需要显式配置以支持更长输入：
    //   - n_seq_max=1: 嵌入只需单序列（decoder 默认 2 会将上下文减半）
    //   - n_batch=8192: 增大可用上下文窗口
    //   - n_ubatch=512: 必须显式设置，否则 embellama 自动推导为 n_batch 同值导致内存溢出
    //
    // 资源优化（桌面应用环境）：
    //   - memory_limit_mb: 硬性内存上限，防止 OOM 导致整个应用崩溃
    //     qwen3-embedding-0.6b Q8_0 模型约 650MB，预留 KV cache + 推理开销
    //   - n_threads: 限制线程数为可用核心的 1/2（至少 1），留余量给 UI 和其他任务
    //   - cache_enabled: 对话搜索中大量重复/相似查询命中缓存，减少推理调用
    //   - use_mmap: 内存映射按需加载，降低启动时间和 RSS 占用
    //   - use_mlock: 不锁定物理内存，友好多任务桌面环境
    let available_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let embed_threads = (available_threads / 2).max(1);

    let engine_config = EngineConfig::builder()
        .with_model_config(model_config)
        .with_n_batch(8192)
        .with_n_seq_max(1)
        .with_n_ubatch(512)
        .with_memory_limit_mb(2048)
        .with_n_threads(embed_threads)
        .with_cache_enabled()
        .with_use_mmap(true)
        .with_use_mlock(false)
        .build()
        .map_err(|e| anyhow::anyhow!("Build engine config failed: {}", e))?;

    let engine = EmbeddingEngine::new(engine_config)
        .map_err(|e| anyhow::anyhow!("Create embedding engine failed: {}", e))?;

    let model_info = engine
        .model_info("qwen3-embedding")
        .map_err(|e| anyhow::anyhow!("Get model info failed: {}", e))?;
    let dimension = model_info.dimensions;

    Ok((engine, dimension))
}

/// Worker 线程主循环：在专用线程中加载模型并处理所有编码请求。
///
/// 模型在此线程的 TLS 中常驻，不会因线程切换而重复加载。
fn worker_thread(
    mut receiver: mpsc::Receiver<EmbedRequest>,
    engine: Mutex<EmbeddingEngine>,
) {
    // Worker 线程入口：模型已在 create_engine 中加载到当前线程的 TLS
    while let Some(request) = receiver.blocking_recv() {
        match request {
            EmbedRequest::Query { text, response } => {
                let result = do_encode_query(&engine, &text);
                let _ = response.send(result);
            }
            EmbedRequest::Passage { text, response } => {
                let result = do_encode_passage(&engine, &text);
                let _ = response.send(result);
            }
            EmbedRequest::Batch { texts, response } => {
                let result = do_encode_batch(&engine, &texts);
                let _ = response.send(result);
            }
        }
    }
    eprintln!("[qwen3-embedder] Worker thread exited (channel closed)");
}

/// 同步编码查询文本，使用 Qwen3 指令模板。
fn do_encode_query(engine: &Mutex<EmbeddingEngine>, text: &str) -> anyhow::Result<Vec<f32>> {
    let prefixed = format!("<Instruct>:{}<Query>:{}", INSTRUCT_PREFIX, text);
    let guard = engine.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
    guard
        .embed(Some("qwen3-embedding"), &prefixed)
        .map_err(|e| anyhow::anyhow!("Embed query failed: {}", e))
}

/// 同步编码段落文本，使用 Qwen3 指令模板。
fn do_encode_passage(engine: &Mutex<EmbeddingEngine>, text: &str) -> anyhow::Result<Vec<f32>> {
    let prefixed = format!("<Instruct>:{}<Document>:{}", INSTRUCT_PREFIX, text);
    let guard = engine.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
    guard
        .embed(Some("qwen3-embedding"), &prefixed)
        .map_err(|e| anyhow::anyhow!("Embed passage failed: {}", e))
}

/// 同步批量编码。
fn do_encode_batch(engine: &Mutex<EmbeddingEngine>, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
    let mut results = Vec::with_capacity(texts.len());
    for text in texts {
        results.push(do_encode_query(engine, text)?);
    }
    Ok(results)
}

impl Qwen3Embedder {
    /// 创建 Qwen3 嵌入器实例。
    ///
    /// 启动专用 Worker 线程并在其中加载 GGUF 模型。
    /// 模型常驻于 Worker 线程的 TLS 中，后续所有编码请求均在该线程处理，
    /// 避免重复加载。
    ///
    /// # Errors
    ///
    /// - 模型文件不存在或加载失败
    /// - 引擎初始化失败
    pub fn new() -> anyhow::Result<Self> {
        let model_path = resolve_model_path();

        if !model_path.exists() {
            anyhow::bail!("Model not found at {}", model_path.display());
        }

        // 创建 channel（buffer=256 足够应对批量向量化场景）
        let (sender, mut receiver) = mpsc::channel::<EmbedRequest>(256);

        // 启动专用 Worker 线程，在其中加载模型并常驻 TLS
        // 关键：引擎必须在 Worker 线程中创建，否则模型存在于调用者线程的 TLS，
        // Worker 线程的 TLS 为空会导致每次 embed 都重新加载模型
        let handle = std::thread::Builder::new()
            .name("qwen3-embed-worker".to_string())
            .spawn(move || {
                match Self::create_engine_in_worker(&model_path) {
                    Ok((worker_engine, dim)) => {
                        eprintln!(
                            "[qwen3-embedder] Worker ready, dimension={}, thread_id={:?}",
                            dim,
                            std::thread::current().id()
                        );
                        worker_thread(receiver, Mutex::new(worker_engine));
                    }
                    Err(e) => {
                        eprintln!("[qwen3-embedder] Worker failed to init: {}", e);
                        // 无法恢复：所有后续请求返回错误
                        while let Some(request) = receiver.blocking_recv() {
                            let err = Err(anyhow::anyhow!("Worker initialization failed: {}", e));
                            match request {
                                EmbedRequest::Query { response, .. }
                                | EmbedRequest::Passage { response, .. } => {
                                    let _ = response.send(err);
                                }
                                EmbedRequest::Batch { response, .. } => {
                                    // 需要类型转换
                                    let _ = response.send(err.map(|_| vec![]).map_err(
                                        |e| anyhow::anyhow!("{:}", e),
                                    ));
                                }
                            }
                        }
                    }
                }
            })
            .map_err(|e| anyhow::anyhow!("Failed to spawn worker thread: {}", e))?;

        // detach 让 Worker 线程独立运行（生命周期随进程）
        let _ = handle;

        // dimension 需要从 Worker 获取，但此时 Worker 可能还未完成初始化。
        // 使用固定值：qwen3-embedding-0.6b 输出维度恒为 1024。
        // Worker 就绪后会打印实际 dimension 到日志供验证。
        Ok(Self {
            sender,
            dimension: 1024,
            context_window: 32768,
        })
    }

    /// 在 Worker 线程内部创建引擎（确保模型加载到 Worker 的 TLS）。
    fn create_engine_in_worker(model_path: &PathBuf) -> anyhow::Result<(EmbeddingEngine, usize)> {
        let model_config = build_model_config(model_path)?;
        create_engine(model_config)
    }
}

#[async_trait]
impl Embedder for Qwen3Embedder {
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(EmbedRequest::Batch {
                texts: texts.iter().map(|t| t.to_string()).collect(),
                response: response_tx,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Worker channel closed: {}", e))?;

        response_rx.await.map_err(|e| anyhow::anyhow!("Worker response dropped: {}", e))?
    }

    async fn encode_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.encode_query(text).await
    }

    async fn encode_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(EmbedRequest::Query {
                text: text.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Worker channel closed: {}", e))?;

        response_rx.await.map_err(|e| anyhow::anyhow!("Worker response dropped: {}", e))?
    }

    async fn encode_passage(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let (response_tx, response_rx) = oneshot::channel();
        self.sender
            .send(EmbedRequest::Passage {
                text: text.to_string(),
                response: response_tx,
            })
            .await
            .map_err(|e| anyhow::anyhow!("Worker channel closed: {}", e))?;

        response_rx.await.map_err(|e| anyhow::anyhow!("Worker response dropped: {}", e))?
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn context_window(&self) -> usize {
        self.context_window
    }

    fn name(&self) -> &str {
        "qwen3-embedding-0.6b"
    }
}
