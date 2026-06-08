//! 文本向量化（Embedding）抽象层。
//!
//! 提供 [`Embedder`] trait 统一嵌入器接口，支持多种后端：
//! - `e5`：基于 multilingual-e5-base GGUF 的本地嵌入器（embellama 推理）
//! - `ollama`：通过 Ollama API 的远程嵌入器
//!
//! E5 后端区分 `encode_query`（检索）和 `encode_passage`（存储），
//! 使用不同前缀以匹配模型训练方式。
//!
//! # 异步设计
//!
//! `Embedder` trait 使用 `#[async_trait]` 标注，所有编码方法均为异步。
//! 这是因为 Ollama 后端需要发起 HTTP 请求，而 E5 后端通过
//! `spawn_blocking` 在独立线程池中执行 CPU 密集推理，天然支持 `.await`。

pub mod e5;
pub mod manager;

pub use manager::EmbedManager;

use async_trait::async_trait;
use once_cell::sync::OnceCell;
use std::sync::Arc;

/// 嵌入器 trait，定义文本向量化接口。
///
/// 提供批量编码、单条编码、查询编码、段落编码等异步方法，
/// 以及维度和名称的同步查询。
#[async_trait]
pub trait Embedder: Send + Sync {
    /// 批量编码多条文本，返回对应的嵌入向量列表。
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>>;

    /// 编码单条文本，返回嵌入向量。
    async fn encode_one(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// 编码查询文本，默认委托给 `encode_one`。
    async fn encode_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.encode_one(text).await
    }

    /// 编码段落文本，默认委托给 `encode_one`。
    async fn encode_passage(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.encode_one(text).await
    }

    /// 返回嵌入向量的维度。
    fn dimension(&self) -> usize;

    /// 返回嵌入器的名称标识。
    fn name(&self) -> &str;
}

static EMBEDDER: OnceCell<Arc<dyn Embedder>> = OnceCell::new();

/// 初始化全局嵌入器单例。
///
/// # Errors
///
/// 重复调用时返回错误。
pub fn init_embedder(embedder: Arc<dyn Embedder>) -> anyhow::Result<()> {
    EMBEDDER
        .set(embedder)
        .map_err(|_| anyhow::anyhow!("Embedder already initialized"))
}

/// 获取全局嵌入器单例的引用。
pub fn get_embedder() -> Option<Arc<dyn Embedder>> {
    EMBEDDER.get().cloned()
}
