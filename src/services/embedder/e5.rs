//! multilingual-e5-base GGUF 嵌入器实现。
//!
//! 通过 embellama（封装 llama-cpp-2）加载量化模型，
//! 使用 Mean Pooling + L2 归一化，输出 768 维句子嵌入向量。
//!
//! E5 模型要求输入文本带前缀：
//! - 检索（query）：`"query: "` 前缀
//! - 存储（passage）：`"passage: "` 前缀
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

/// 基于 multilingual-e5-base GGUF Q8_0 的本地嵌入器。
///
/// 使用 embellama（封装 llama-cpp-2）推理引擎，通过 Arc<Mutex> + spawn_blocking
/// 实现异步安全的嵌入计算。
pub struct E5Embedder {
    engine: Arc<Mutex<EmbeddingEngine>>,
    dimension: usize,
}

/// 解析模型文件路径。
///
/// 按以下顺序尝试查找：
/// 1. 标准数据目录：由模型下载服务下载到的位置（跨平台标准路径）
/// 2. 开发环境：当前工作目录 / `assets/models/multilingual-e5-base-q8_0.gguf`
/// 3. macOS 打包：`.app/Contents/Resources/assets/models/`
/// 4. 其他平台打包：可执行文件同级目录 / `assets/models/`
fn resolve_model_path() -> PathBuf {
    let model_name = "multilingual-e5-base-q8_0.gguf";

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
        .join(model_name);
    if dev_path.exists() {
        return dev_path;
    }

    // 3. 从可执行文件路径推导（打包后）
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_dir = exe_path.parent().unwrap_or_else(|| std::path::Path::new("."));

        // macOS: XEChat.app/Contents/MacOS/xechat -> ../Resources/assets/models/
        #[cfg(target_os = "macos")]
        {
            let macos_resources = exe_dir
                .parent()
                .map(|p| p.join("Resources"))
                .unwrap_or_default()
                .join("assets")
                .join("models")
                .join(model_name);
            if macos_resources.exists() {
                return macos_resources;
            }
        }

        // Windows/Linux: 与 exe 同级
        let bundled_path = exe_dir
            .join("assets")
            .join("models")
            .join(model_name);
        if bundled_path.exists() {
            return bundled_path;
        }
    }

    // 兜底：返回标准数据目录路径（不存在时会报错，提示用户下载模型）
    data_path
}

impl E5Embedder {
    /// 创建 E5 嵌入器实例。
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

        let model_config = ModelConfig::builder()
            .with_model_path(model_path.to_str().unwrap_or_default())
            .with_model_name("e5-base")
            .with_normalization_mode(NormalizationMode::L2)
            .with_pooling_strategy(PoolingStrategy::Mean)
            .build()
            .map_err(|e| anyhow::anyhow!("Build model config failed: {}", e))?;

        let engine_config = EngineConfig::builder()
            .with_model_config(model_config)
            .build()
            .map_err(|e| anyhow::anyhow!("Build engine config failed: {}", e))?;

        let engine = EmbeddingEngine::new(engine_config)
            .map_err(|e| anyhow::anyhow!("Create embedding engine failed: {}", e))?;

        // 获取嵌入维度
        let model_info = engine
            .model_info("e5-base")
            .map_err(|e| anyhow::anyhow!("Get model info failed: {}", e))?;
        let dimension = model_info.dimensions;

        eprintln!(
            "[e5-embedder] Model loaded (embellama), dimension={}",
            dimension
        );

        Ok(Self {
            engine: Arc::new(Mutex::new(engine)),
            dimension,
        })
    }

    /// 同步编码带前缀的文本。
    fn do_encode(engine: &Mutex<EmbeddingEngine>, prefix: &str, text: &str) -> anyhow::Result<Vec<f32>> {
        let prefixed = format!("{}{}", prefix, text);
        let guard = engine.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {}", e))?;
        guard
            .embed(Some("e5-base"), &prefixed)
            .map_err(|e| anyhow::anyhow!("Embed text failed: {}", e))
    }
}

#[async_trait]
impl Embedder for E5Embedder {
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
        tokio::task::spawn_blocking(move || Self::do_encode(&engine, "query: ", &text))
            .await
            .map_err(|e| anyhow::anyhow!("Spawn blocking failed: {}", e))?
    }

    async fn encode_passage(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let engine = self.engine.clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || Self::do_encode(&engine, "passage: ", &text))
            .await
            .map_err(|e| anyhow::anyhow!("Spawn blocking failed: {}", e))?
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn name(&self) -> &str {
        "multilingual-e5-base-q8_0"
    }
}
