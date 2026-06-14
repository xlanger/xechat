//! 语义分块与向量工具函数。
//!
//! 提供 [`ChunkParams`] 动态分块参数计算、[`semantic_chunk`] 语义边界切分、
//! [`normalize_vector`] L2 归一化等工具函数，供嵌入器和记忆管线使用。

/// 角色标签行前缀，切分时不可在这些行中间截断。
const ROLE_LABELS: &[&str] = &["用户：", "助手："];

/// 模型 token 开销（BOS/EOS/特殊 token/前缀等）。
const TOKEN_OVERHEAD: usize = 258;

/// 中文平均 token/字符比。
const AVG_TOKENS_PER_CHAR: f64 = 1.75;

/// 分块跨度
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkSpan {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// 动态分块参数，根据 embedder 的 context_window 计算。
#[derive(Debug, Clone, Copy)]
pub struct ChunkParams {
    /// 目标分块字符数
    pub target_chars: usize,
    /// 最大分块字符数
    pub max_chars: usize,
    /// 重叠字符数
    pub overlap_chars: usize,
}

impl ChunkParams {
    /// 根据 embedder 的 context_window 动态计算分块参数。
    ///
    /// 计算公式：
    /// - effective_tokens = context_window - overhead
    /// - target_chars = effective_tokens * 0.6 / avg_tokens_per_char
    /// - max_chars = effective_tokens * 0.8 / avg_tokens_per_char
    /// - overlap_chars = target_chars * 0.2
    pub fn from_context_window(context_window: usize) -> Self {
        let effective_tokens = context_window.saturating_sub(TOKEN_OVERHEAD);
        let target_chars = (effective_tokens as f64 * 0.6 / AVG_TOKENS_PER_CHAR) as usize;
        let max_chars = (effective_tokens as f64 * 0.8 / AVG_TOKENS_PER_CHAR) as usize;
        let overlap_chars = (target_chars as f64 * 0.2) as usize;
        Self {
            target_chars: target_chars.max(50),
            max_chars: max_chars.max(target_chars + 20),
            overlap_chars: overlap_chars.max(10),
        }
    }
}

impl Default for ChunkParams {
    fn default() -> Self {
        // Qwen3-Embedding (32K tokens) 的默认参数
        Self::from_context_window(32768)
    }
}

/// 计算下一个分块的起始位置（含重叠回退）。
///
/// 从当前块的结束位置回退 `overlap` 个字符，确保分块间有上下文重叠。
#[inline]
pub fn compute_next_chunk_start(text: &str, end: usize, overlap: usize) -> usize {
    if end >= overlap {
        char_offset_back(&text[..end], overlap)
    } else {
        0
    }
}

/// 按语义边界切分文本。
///
/// 优先级：角色标签边界 > 段落边界 > 句子边界 > 字符滑动窗口。
/// 分块参数由 `ChunkParams` 动态指定。
/// 不允许在角色标签行（"用户："、"助手："）中间切分。
pub fn semantic_chunk(text: &str, params: ChunkParams) -> Vec<ChunkSpan> {
    if text.chars().count() <= params.target_chars {
        return vec![ChunkSpan { text: text.to_string(), start: 0, end: text.len() }];
    }

    let mut chunks = Vec::new();
    let mut pos = 0;

    while pos < text.len() {
        let remaining = &text[pos..];
        let target_end = pos + char_offset(remaining, params.target_chars);

        if target_end >= text.len() {
            chunks.push(ChunkSpan { text: text[pos..].to_string(), start: pos, end: text.len() });
            break;
        }

        // 在目标位置附近找语义边界
        let boundary = find_boundary(&text[pos..], params.target_chars, params.max_chars);

        let end = pos + boundary;
        chunks.push(ChunkSpan { text: text[pos..end].to_string(), start: pos, end });

        // 下一块起始位置：回退 overlap
        pos = compute_next_chunk_start(text, end, params.overlap_chars);
    }

    chunks
}

/// 在目标长度附近查找最佳语义边界，返回切分点（字节偏移，相对于 text 起点）。
fn find_boundary(text: &str, target: usize, max: usize) -> usize {
    let candidate = find_boundary_core(text, target, max);
    protect_role_labels(text, candidate)
}

/// 在搜索范围内查找最佳句子边界位置。
pub fn find_sentence_boundary(text: &str, search_start: usize, max_off: usize) -> Option<usize> {
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
    if best > 0 { Some(best) } else { None }
}

/// 在搜索范围内查找角色标签行之前的换行位置。
pub fn find_role_label_boundary(text: &str, search_start: usize, max_off: usize) -> Option<usize> {
    let pos = find_last_occurrence(text, "\n", search_start, max_off)?;
    let after_newline = &text[pos + 1..];
    if ROLE_LABELS.iter().any(|label| after_newline.starts_with(label)) {
        Some(pos + 1)
    } else {
        None
    }
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
    if let Some(best) = find_sentence_boundary(text, search_start, max_off) {
        return best;
    }

    // 3. 找换行边界（角色标签行之间的换行）
    if let Some(pos) = find_role_label_boundary(text, search_start, max_off) {
        return pos;
    }

    // 4. 回退到目标长度硬切
    target_off
}

/// 检查切分点是否落在某个标签内部，若是则返回该标签前的换行位置。
pub fn find_label_overlap_boundary(text: &str, cut_point: usize, label: &str) -> Option<usize> {
    let search_start = floor_char_boundary(text, cut_point.saturating_sub(label.len()));
    let search_end = ceil_char_boundary(text, (cut_point + label.len()).min(text.len()));
    if search_end > text.len() {
        return None;
    }
    let window = &text[search_start..search_end];
    let relative_pos = window.find(label)?;
    let label_start = search_start + relative_pos;
    if cut_point > label_start && cut_point < label_start + label.len() {
        Some(text[..label_start].rfind('\n').map(|p| p + 1).unwrap_or(0))
    } else {
        None
    }
}

/// 角色标签保护：确保切分点不在角色标签行中间。
fn protect_role_labels(text: &str, cut_point: usize) -> usize {
    for label in ROLE_LABELS {
        if let Some(boundary) = find_label_overlap_boundary(text, cut_point, label) {
            return boundary;
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

/// 对向量进行 L2 归一化（原地修改）。
pub fn normalize_vector(vec: &mut [f32]) {
    let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
}

