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
//! # 线程安全设计
//!
//! embellama 底层使用 llama-cpp-2，其 `LlamaContext` 是 `!Send`。
//! 因此使用 `Arc<Mutex<EmbeddingEngine>>` + `spawn_blocking` 模式，
//! 在独立线程中执行推理，避免阻塞异步运行时。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use embellama::{
    EmbeddingEngine, EngineConfig, ModelConfig, NormalizationMode, PoolingStrategy,
};

use super::Embedder;

/// Qwen3-Embedding 指令前缀（检索任务）。
const INSTRUCT_PREFIX: &str = "检索相关文章";

/// 模型文件名。
const MODEL_FILENAME: &str = "qwen3-embedding-0.6b-q8_0.gguf";

/// 基于 Qwen3-Embedding-0.6B GGUF 的本地嵌入器。
///
/// 使用 embellama（封装 llama-cpp-2）推理引擎，通过 Arc<Mutex> + spawn_blocking
/// 实现异步安全的嵌入计算。
pub struct Qwen3Embedder {
    engine: Arc<Mutex<EmbeddingEngine>>,
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
        .with_pooling_strategy(PoolingStrategy::Mean)
        .build()
        .map_err(|e| anyhow::anyhow!("Build model config failed: {}", e))
}

/// 构建引擎配置并创建嵌入引擎。
fn create_engine(model_config: ModelConfig) -> anyhow::Result<(EmbeddingEngine, usize)> {
    let engine_config = EngineConfig::builder()
        .with_model_config(model_config)
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

impl Qwen3Embedder {
    /// 创建 Qwen3 嵌入器实例。
    ///
    /// 加载 GGUF 模型并初始化 embellama 推理引擎。
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

        let model_config = build_model_config(&model_path)?;
        let (engine, dimension) = create_engine(model_config)?;

        eprintln!(
            "[qwen3-embedder] Model loaded (embellama), dimension={}",
            dimension
        );

        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            dimension,
            context_window: 32768,
        })
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
}

#[async_trait]
impl Embedder for Qwen3Embedder {
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.encode_one(text).await?);
        }
        Ok(results)
    }

    async fn encode_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.encode_query(text).await
    }

    async fn encode_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let engine = self.engine.clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || Self::do_encode_query(&engine, &text))
            .await
            .map_err(|e| anyhow::anyhow!("Spawn blocking failed: {}", e))?
    }

    async fn encode_passage(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let engine = self.engine.clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || Self::do_encode_passage(&engine, &text))
            .await
            .map_err(|e| anyhow::anyhow!("Spawn blocking failed: {}", e))?
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
