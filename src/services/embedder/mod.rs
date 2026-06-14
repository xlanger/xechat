//! 文本向量化（Embedding）抽象层。
//!
//! 提供 [`Embedder`] trait 统一嵌入器接口，支持多种后端：
//! - `qwen3`：基于 Qwen3-Embedding-0.6B GGUF 的本地嵌入器（embellama 推理）
//! - `ollama`：通过 Ollama API 的远程嵌入器（Qwen3-Embedding-4B/8B）
//!
//! Qwen3-Embedding 使用统一的指令模板区分查询和段落编码：
//! - 查询：`<Instruct>:检索相关文章<Query>:文本`
//! - 段落：`<Instruct>:检索相关文章<Document>:文本`
//!
//! # 异步设计
//!
//! `Embedder` trait 使用 `#[async_trait]` 标注，所有编码方法均为异步。
//! 这是因为 Ollama 后端需要发起 HTTP 请求，而 Qwen3 后端通过
//! `spawn_blocking` 在独立线程池中执行 CPU 密集推理，天然支持 `.await`。

pub mod qwen3;
pub mod manager;

pub use manager::{ChunkParams, find_sentence_boundary, find_role_label_boundary, find_label_overlap_boundary, normalize_vector};

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::RwLock;

/// 嵌入器 trait，定义文本向量化接口。
///
/// 提供批量编码、单条编码、查询编码、段落编码等异步方法，
/// 以及维度、上下文窗口和名称的同步查询。
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

    /// 批量编码段落文本，默认逐条调用 `encode_passage`。
    ///
    /// Ollama 等远程后端应覆写此方法，利用 `/api/embed` 批量接口
    /// 一次发送多个文本，减少 HTTP 请求次数。
    async fn encode_passages(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.encode_passage(text).await?);
        }
        Ok(results)
    }

    /// 返回嵌入向量的维度。
    fn dimension(&self) -> usize;

    /// 返回模型的最大上下文窗口（token 数）。
    ///
    /// 用于动态计算分块参数。Qwen3-Embedding-0.6B 为 32768，ollama 模型通常为 8192。
    /// 默认返回 512。
    fn context_window(&self) -> usize {
        512
    }

    /// 返回嵌入器的名称标识。
    fn name(&self) -> &str;
}

static EMBEDDER: RwLock<Option<Arc<dyn Embedder>>> = RwLock::new(None);

/// 设置全局嵌入器。
///
/// 首次初始化和运行时切换嵌入提供商时调用。
pub fn init_embedder(embedder: Arc<dyn Embedder>) -> anyhow::Result<()> {
    let mut guard = EMBEDDER.write().map_err(|e| anyhow::anyhow!("Embedder lock poisoned: {}", e))?;
    *guard = Some(embedder);
    Ok(())
}

/// 获取全局嵌入器的引用。
pub fn get_embedder() -> Option<Arc<dyn Embedder>> {
    EMBEDDER
        .read()
        .ok()
        .and_then(|guard| guard.clone())
}
