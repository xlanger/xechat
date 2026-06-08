//! Ollama 嵌入扩展实现。
//!
//! 通过 Ollama `/api/embed` 接口进行文本向量化，
//! 支持任意 Ollama 嵌入模型（Jina、Nomic、GTE 等）。
//!
//! # 使用方式
//!
//! - [`OllamaEmbedder::probe()`]：异步探测 Ollama 服务并验证模型可用性，
//!   成功时自动获取向量维度，推荐用于初始化。
//! - [`OllamaEmbedder::new()`]：同步构造器，不验证可用性，
//!   适用于已知 Ollama 可用的场景。

use async_trait::async_trait;
use reqwest::Client;

use crate::services::embedder::Embedder;

/// Ollama 嵌入扩展，通过 `/api/embed` 接口进行文本向量化。
///
/// 支持 Jina、Nomic、GTE 等任意 Ollama 嵌入模型。
/// 维度在 `probe()` 时自动检测，或通过 `new()` 手动指定。
pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    dimension: usize,
    client: Client,
}

impl OllamaEmbedder {
    /// 同步构造器，不验证 Ollama 服务可用性。
    ///
    /// 使用默认 768 维度，实际维度可能因模型不同而异。
    /// 推荐使用 [`probe()`](Self::probe) 自动检测。
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
            dimension: 768,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    /// 异步探测 Ollama 服务并验证模型可用性。
    ///
    /// 发送测试嵌入请求，成功时自动获取向量维度。
    /// 用于初始化阶段确认 Ollama 可用，不可用时返回错误以便降级到 E5。
    ///
    /// # Errors
    ///
    /// - Ollama 服务不可达
    /// - 指定模型不存在或未加载
    /// - 响应格式异常
    pub async fn probe(base_url: &str, model: &str) -> anyhow::Result<Self> {
        let probe_client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        let resp = probe_client
            .post(format!("{}/api/embed", base_url))
            .json(&serde_json::json!({"model": model, "input": ["test"]}))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama embed probe failed ({}): {}", status, body);
        }

        let body: serde_json::Value = resp.json().await?;
        let dimension = body["embeddings"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|e| e.as_array())
            .map(|arr| arr.len())
            .unwrap_or(768);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
            dimension,
            client,
        })
    }
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let resp = self
            .client
            .post(format!("{}/api/embed", self.base_url))
            .json(&serde_json::json!({
                "model": self.model,
                "input": texts
            }))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Ollama embed failed ({}): {}", status, body);
        }

        let body: serde_json::Value = resp.json().await?;
        let embeddings = body["embeddings"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No embeddings in Ollama response"))?;

        let result: Vec<Vec<f32>> = embeddings
            .iter()
            .map(|e| {
                e.as_array()
                    .ok_or_else(|| anyhow::anyhow!("Invalid embedding entry"))
                    .and_then(|arr| {
                        arr.iter()
                            .map(|v| {
                                v.as_f64()
                                    .ok_or_else(|| anyhow::anyhow!("Invalid float in embedding"))
                                    .map(|f| f as f32)
                            })
                            .collect::<anyhow::Result<Vec<f32>>>()
                    })
            })
            .collect::<anyhow::Result<Vec<Vec<f32>>>>()?;

        Ok(result)
    }

    async fn encode_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut result = self.encode(&[text]).await?;
        result
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Empty result from Ollama"))
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn name(&self) -> &str {
        "ollama"
    }
}
