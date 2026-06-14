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
/// 支持任意 Ollama 嵌入模型（如 Qwen3-Embedding-4B/8B）。
/// 维度在 `probe()` 时自动检测，或通过 `new()` 手动指定。
///
/// # 指令模板策略
///
/// Qwen3-Embedding 系列要求查询和段落使用不同的指令模板：
/// - 查询：`<Instruct>:检索相关文章<Query>:文本`
/// - 段落：`<Instruct>:检索相关文章<Document>:文本`
///
/// Ollama 的 `/api/embed` 接口不会自动添加指令模板，
/// 因此需要手动添加，确保与内置 Qwen3Embedder 产生一致的向量空间。
pub struct OllamaEmbedder {
    base_url: String,
    model: String,
    dimension: usize,
    /// 模型上下文窗口大小（token 数），用于分块参数计算。
    context_window: usize,
    /// 缓存的嵌入器名称，格式 "ollama:{model}"
    name: String,
    /// 是否为 Qwen3-Embedding 系列模型（需要指令模板）。
    is_qwen3_embed: bool,
    client: Client,
}

impl OllamaEmbedder {
    /// 检测模型是否为 Qwen3-Embedding 系列。
    ///
    /// Qwen3-Embedding 模型名包含 "qwen3" 和 "embed"。
    fn is_qwen3_embedding(model: &str) -> bool {
        let lower = model.to_lowercase();
        lower.contains("qwen3") && lower.contains("embed")
    }

    /// 同步构造器，不验证 Ollama 服务可用性。
    ///
    /// 使用默认 768 维度，实际维度可能因模型不同而异。
    /// 推荐使用 [`probe()`](Self::probe) 自动检测。
    pub fn new(base_url: &str, model: &str) -> Self {
        let is_qwen3_embed = Self::is_qwen3_embedding(model);
        Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
            dimension: 1024,
            context_window: 8192,
            name: format!("ollama:{}", model),
            is_qwen3_embed,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
        }
    }

    /// 从 Ollama 嵌入响应中提取向量维度。
    pub fn extract_dimension(body: &serde_json::Value) -> usize {
        body["embeddings"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|e| e.as_array())
            .map(|arr| arr.len())
            .unwrap_or(768)
    }

    /// 从 Ollama 嵌入响应中解析嵌入向量列表。
    pub fn parse_embeddings(body: &serde_json::Value) -> anyhow::Result<Vec<Vec<f32>>> {
        let embeddings = body["embeddings"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("No embeddings in Ollama response"))?;

        embeddings
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
            .collect::<anyhow::Result<Vec<Vec<f32>>>>()
    }

    /// 创建短超时的探测用 HTTP 客户端。
    fn create_probe_client() -> anyhow::Result<Client> {
        Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build probe client: {}", e))
    }

    /// 创建标准超时的 HTTP 客户端。
    fn create_standard_client() -> anyhow::Result<Client> {
        Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build standard client: {}", e))
    }

    /// 异步探测 Ollama 服务并验证模型可用性。
    ///
    /// 发送测试嵌入请求，成功时自动获取向量维度。
    /// 同时查询模型信息获取上下文窗口大小。
    /// 用于初始化阶段确认 Ollama 可用，不可用时返回错误以便降级到 Qwen3-Embedding。
    ///
    /// # Errors
    ///
    /// - Ollama 服务不可达
    /// - 指定模型不存在或未加载
    /// - 响应格式异常
    pub async fn probe(base_url: &str, model: &str) -> anyhow::Result<Self> {
        let probe_client = Self::create_probe_client()?;

        let resp = send_probe_request(&probe_client, base_url, model).await?;
        let body: serde_json::Value = resp.json().await?;
        let dimension = Self::extract_dimension(&body);

        // 查询模型信息获取上下文窗口
        let context_window = fetch_model_context_window(&probe_client, base_url, model).await;

        let client = Self::create_standard_client()?;

        let is_qwen3_embed = Self::is_qwen3_embedding(model);

        // Ollama 本地推理速度远低于服务端 GPU，限制有效 context_window
        // 使分块更积极，避免长文本嵌入超时。
        // 8192 tokens ≈ 2721 字符 target_chars，大部分对话轮次可保持完整
        let effective_context_window = context_window.min(8192);

        Ok(Self {
            base_url: base_url.to_string(),
            model: model.to_string(),
            dimension,
            context_window: effective_context_window,
            name: format!("ollama:{}", model),
            is_qwen3_embed,
            client,
        })
    }

    /// 发送嵌入请求并验证响应状态码。
    pub async fn send_encode_request(
        &self,
        texts: &[&str],
    ) -> anyhow::Result<reqwest::Response> {
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

        Ok(resp)
    }
}

/// 发送探测请求到 Ollama 嵌入接口。
///
/// 返回成功响应，失败时返回包含状态码和响应体的错误。
pub async fn send_probe_request(
    client: &Client,
    base_url: &str,
    model: &str,
) -> anyhow::Result<reqwest::Response> {
    let resp = client
        .post(format!("{}/api/embed", base_url))
        .json(&serde_json::json!({"model": model, "input": ["test"]}))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Ollama embed probe failed ({}): {}", status, body);
    }

    Ok(resp)
}

/// 从 Ollama `/api/show` 接口查询模型的上下文窗口大小。
///
/// 解析 `model_info` 中的 `context_length` 字段。
/// 查询失败时返回默认值 8192。
async fn fetch_model_context_window(client: &Client, base_url: &str, model: &str) -> usize {
    let url = format!("{}/api/show", base_url);
    let resp = match client
        .post(&url)
        .json(&serde_json::json!({"name": model}))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return 8192,
    };

    if !resp.status().is_success() {
        return 8192;
    }

    let body: serde_json::Value = match resp.json().await {
        Ok(b) => b,
        Err(_) => return 8192,
    };

    // 尝试从 model_info 中提取 context_length
    body.get("model_info")
        .and_then(|info| {
            // 查找包含 context_length 的键
            info.as_object()?.iter().find_map(|(k, v)| {
                if k.ends_with("context_length") {
                    v.as_u64()
                } else {
                    None
                }
            })
        })
        .unwrap_or(8192) as usize
}

#[async_trait]
impl Embedder for OllamaEmbedder {
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let resp = self.send_encode_request(texts).await?;
        let body: serde_json::Value = resp.json().await?;
        Self::parse_embeddings(&body)
    }

    async fn encode_one(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let mut result = self.encode(&[text]).await?;
        result
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Empty result from Ollama"))
    }

    /// 编码查询文本，Qwen3-Embedding 模型自动添加指令模板。
    ///
    /// Qwen3-Embedding 要求查询文本使用 `<Instruct>:...<Query>:` 模板，
    /// 与内置 Qwen3Embedder 保持一致，确保向量空间匹配。
    async fn encode_query(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        if self.is_qwen3_embed {
            let prefixed = format!("<Instruct>:检索相关文章<Query>:{}", text);
            eprintln!("[xechat:embed] Ollama encode_query with Qwen3 prefix: '{}...'", &prefixed[..40.min(prefixed.len())]);
            self.encode_one(&prefixed).await
        } else {
            self.encode_one(text).await
        }
    }

    /// 编码段落文本，Qwen3-Embedding 模型自动添加指令模板。
    ///
    /// Qwen3-Embedding 要求段落文本使用 `<Instruct>:...<Document>:` 模板，
    /// 与内置 Qwen3Embedder 保持一致，确保向量空间匹配。
    async fn encode_passage(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        if self.is_qwen3_embed {
            let prefixed = format!("<Instruct>:检索相关文章<Document>:{}", text);
            self.encode_one(&prefixed).await
        } else {
            self.encode_one(text).await
        }
    }

    /// 批量编码段落文本，一次 HTTP 请求发送所有文本。
    ///
    /// 利用 Ollama `/api/embed` 的批量接口，将多个文本合并为一次请求，
    /// 大幅减少 HTTP 往返次数。Qwen3-Embedding 模型会自动添加指令模板。
    async fn encode_passages(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let prefixed_texts: Vec<String> = if self.is_qwen3_embed {
            texts.iter()
                .map(|t| format!("<Instruct>:检索相关文章<Document>:{}", t))
                .collect()
        } else {
            texts.iter().map(|t| t.to_string()).collect()
        };

        let refs: Vec<&str> = prefixed_texts.iter().map(|s| s.as_str()).collect();
        eprintln!("[xechat:embed] Ollama encode_passages batch: {} texts", refs.len());
        self.encode(&refs).await
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn context_window(&self) -> usize {
        self.context_window
    }

    fn name(&self) -> &str {
        &self.name
    }
}
