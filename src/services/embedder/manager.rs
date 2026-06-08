//! 智能编码层：E5 默认 + Ollama 可选增强。
//!
//! 短文本（<400 字符）使用 E5 直接处理，
//! 长文本优先 Ollama（如果可用且有长上下文模型），
//! Ollama 不可用时回退到 E5 语义边界分块编码。

use std::sync::Arc;

use async_trait::async_trait;
use once_cell::sync::OnceCell;

use super::{Embedder, e5::E5Embedder};

/// 语义边界分块参数
///
/// E5-base GGUF Q8_0 的 token 预算：
/// - usable_context = 512 tokens
/// - overhead（BOS/EOS/特殊 token/前缀）≈ 258 tokens
/// - effective_max = 254 tokens
/// - 中文约 1.5-2 tokens/字符 → 254 tokens ≈ 127-170 字符
/// - 轮次格式 "用户：xxx\n助手：yyy" 角色标签约占 10-15 tokens
/// - 实际可用 ≈ 120-150 字符
///
/// 因此分块目标设为 150 字符，最大 200 字符，重叠 30 字符。
const CHUNK_TARGET_CHARS: usize = 150;
const CHUNK_OVERLAP_CHARS: usize = 30;
const CHUNK_MAX_CHARS: usize = 200;

/// 角色标签行前缀，切分时不可在这些行中间截断。
const ROLE_LABELS: &[&str] = &["用户：", "助手："];

/// 分块跨度
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkSpan {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// 智能编码管理器：E5 打底，Ollama 可选增强长文本。
pub struct EmbedManager {
    e5: Arc<E5Embedder>,
    ollama: OnceCell<Arc<dyn Embedder>>,
}

impl EmbedManager {
    /// 创建 EmbedManager，E5 作为默认嵌入器。
    pub fn new(e5: Arc<E5Embedder>) -> Self {
        Self {
            e5,
            ollama: OnceCell::new(),
        }
    }

    /// 激活 Ollama 扩展（探测成功后调用）。
    pub fn enable_ollama(&self, embedder: Arc<dyn Embedder>) -> anyhow::Result<()> {
        self.ollama
            .set(embedder)
            .map_err(|_| anyhow::anyhow!("Ollama already enabled"))
    }

    /// E5 长文本分块编码：语义边界切分 → 逐块编码 → 返回所有分块向量。
    pub async fn encode_long_chunks(&self, text: &str) -> anyhow::Result<Vec<(ChunkSpan, Vec<f32>)>> {
        let spans = semantic_chunk(text);
        let mut results = Vec::with_capacity(spans.len());
        for span in &spans {
            let embedding = self.e5.encode_passage(&span.text).await?;
            results.push((span.clone(), embedding));
        }
        Ok(results)
    }
}

#[async_trait]
impl Embedder for EmbedManager {
    async fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
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
        let char_count = text.chars().count();
        if char_count <= CHUNK_TARGET_CHARS {
            return self.e5.encode_query(text).await;
        }
        if let Some(ollama) = self.ollama.get() {
            return ollama.encode_query(text).await;
        }
        let chunks = self.encode_long_chunks(text).await?;
        Ok(aggregate_mean(
            &chunks.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
        ))
    }

    async fn encode_passage(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        let char_count = text.chars().count();
        if char_count <= CHUNK_TARGET_CHARS {
            return self.e5.encode_passage(text).await;
        }
        if let Some(ollama) = self.ollama.get() {
            return ollama.encode_passage(text).await;
        }
        let chunks = self.encode_long_chunks(text).await?;
        Ok(aggregate_mean(
            &chunks.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
        ))
    }

    fn dimension(&self) -> usize {
        self.e5.dimension()
    }

    fn name(&self) -> &str {
        if self.ollama.get().is_some() {
            "embed-manager+ollama"
        } else {
            "embed-manager+e5"
        }
    }
}

/// 按语义边界切分文本。
///
/// 优先级：角色标签边界 > 段落边界 > 句子边界 > 字符滑动窗口。
/// 每块目标 150 字符，最大 200 字符，重叠 30 字符。
/// 不允许在角色标签行（"用户："、"助手："）中间切分。
pub fn semantic_chunk(text: &str) -> Vec<ChunkSpan> {
    if text.chars().count() <= CHUNK_TARGET_CHARS {
        return vec![ChunkSpan { text: text.to_string(), start: 0, end: text.len() }];
    }

    let mut chunks = Vec::new();
    let mut pos = 0;

    while pos < text.len() {
        let remaining = &text[pos..];
        let target_end = pos + char_offset(remaining, CHUNK_TARGET_CHARS);

        if target_end >= text.len() {
            chunks.push(ChunkSpan { text: text[pos..].to_string(), start: pos, end: text.len() });
            break;
        }

        // 在目标位置附近找语义边界
        let boundary = find_boundary(&text[pos..], CHUNK_TARGET_CHARS, CHUNK_MAX_CHARS);

        let end = pos + boundary;
        chunks.push(ChunkSpan { text: text[pos..end].to_string(), start: pos, end });

        // 下一块起始位置：回退 overlap
        let overlap_start = if end >= CHUNK_OVERLAP_CHARS {
            char_offset_back(&text[..end], CHUNK_OVERLAP_CHARS)
        } else {
            0
        };
        pos = overlap_start;
    }

    chunks
}

/// 在目标长度附近查找最佳语义边界，返回切分点（字节偏移，相对于 text 起点）。
fn find_boundary(text: &str, target: usize, max: usize) -> usize {
    let candidate = find_boundary_core(text, target, max);
    protect_role_labels(text, candidate)
}

/// 核心边界查找逻辑（不含角色标签保护）。
///
/// 优先级：段落 > 句子 > 角色标签行 > 硬切。
/// 优先在语义边界（段落、句子）处切割，保持文本连贯性。
fn find_boundary_core(text: &str, target: usize, max: usize) -> usize {
    let target_off = char_offset(text, target);
    let max_off = char_offset(text, max).min(text.len());
    let search_start = char_offset(text, target * 4 / 5);

    // 1. 在 [target*0.8, max] 范围内找段落边界 \n\n
    if let Some(pos) = find_last_occurrence(text, "\n\n", search_start, max_off) {
        return pos + 2;
    }

    // 2. 找句子边界（。！？.!?）——优先语义切割
    let sentence_endings = ["\u{3002}", "\u{FF01}", "\u{FF1F}", ".", "!", "?"];
    let mut best = 0;
    for ending in &sentence_endings {
        if let Some(pos) = find_last_occurrence(text, ending, search_start, max_off) {
            let candidate = pos + ending.len();
            if candidate > best {
                best = candidate;
            }
        }
    }
    if best > 0 {
        return best;
    }

    // 3. 找换行边界（角色标签行之间的换行）
    if let Some(pos) = find_last_occurrence(text, "\n", search_start, max_off) {
        let after_newline = &text[pos + 1..];
        if ROLE_LABELS.iter().any(|label| after_newline.starts_with(label)) {
            return pos + 1;
        }
    }

    // 4. 回退到目标长度硬切
    target_off
}

/// 角色标签保护：确保切分点不在角色标签行中间。
fn protect_role_labels(text: &str, cut_point: usize) -> usize {
    for label in ROLE_LABELS {
        let search_start = floor_char_boundary(text, cut_point.saturating_sub(label.len()));
        let search_end = ceil_char_boundary(text, (cut_point + label.len()).min(text.len()));
        if search_end > text.len() {
            continue;
        }
        let window = &text[search_start..search_end];
        if let Some(relative_pos) = window.find(label) {
            let label_start = search_start + relative_pos;
            if cut_point > label_start && cut_point < label_start + label.len() {
                if let Some(prev_newline) = text[..label_start].rfind('\n') {
                    return prev_newline + 1;
                }
                return 0;
            }
        }
    }
    cut_point
}

/// 将字节偏移向下对齐到最近的 UTF-8 字符边界。
fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// 将字节偏移向上对齐到最近的 UTF-8 字符边界。
fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// 在 text[start..end] 范围内查找 needle 最后一次出现的位置（字节偏移）
fn find_last_occurrence(text: &str, needle: &str, start: usize, end: usize) -> Option<usize> {
    let slice = &text[start..end.min(text.len())];
    slice.rfind(needle).map(|pos| start + pos)
}

/// 返回从 text 起始偏移 n 个字符对应的字节偏移
fn char_offset(text: &str, n: usize) -> usize {
    text.char_indices()
        .nth(n)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

/// 返回从 text 末尾回退 n 个字符对应的字节偏移
fn char_offset_back(text: &str, n: usize) -> usize {
    let char_count = text.chars().count();
    if n >= char_count {
        return 0;
    }
    text.char_indices()
        .nth(char_count - n)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn aggregate_mean(embeddings: &[Vec<f32>]) -> Vec<f32> {
    if embeddings.is_empty() {
        return Vec::new();
    }
    let dim = embeddings[0].len();
    let mut sum = vec![0.0f32; dim];
    for emb in embeddings {
        for (i, &val) in emb.iter().enumerate() {
            sum[i] += val;
        }
    }
    let n = embeddings.len() as f32;
    let mut result: Vec<f32> = sum.iter().map(|&s| s / n).collect();
    let norm = result.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in result.iter_mut() {
            *v /= norm;
        }
    }
    result
}
