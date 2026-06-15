# XEChat 记忆管线与搜索系统 — 设计规格

> 子项目一：记忆管线（意图判断 → 记忆检索 → Prompt 组装 → 回复 → 向量化持久化）
> 子项目二：搜索界面（LanceDB 全文搜索 + 语义搜索 + 结果展示 + 定位）

> **当前实现状态**：已实现。嵌入模型从原设计的 Jina v2 Zh ONNX 迁移至 Qwen3-Embedding GGUF（embellama 引擎）。

---

## 1. 架构概览

```
┌──────────────────────────────────────────────────────┐
│                    Dioxus 桌面应用                     │
│                                                       │
│  ┌──────────────┐          ┌──────────────────────┐  │
│  │   对话界面    │          │     搜索界面          │  │
│  │ (DeepSeek API│          │ (LanceDB 全文         │  │
│  │  主链路)     │          │  + Qwen3 语义)        │  │
│  └──────┬───────┘          └──────────┬───────────┘  │
│         │                             │               │
│  ┌──────┴─────────────────────────────┴───────────┐  │
│  │           本地核心层（内置，零外部依赖）          │  │
│  │  ┌──────────────────┐  ┌────────────────────┐  │  │
│  │  │ Qwen3-Embedding  │  │   LanceDB 全文索引  │  │  │
│  │  │ 0.6B GGUF (Q8_0) │  │   (消息内容+元数据) │  │  │
│  │  │ ~490MB 嵌入       │  │                     │  │  │
│  │  │ 1024 维向量       │  │                     │  │  │
│  │  └──────┬───────────┘  └──────────┬─────────┘  │  │
│  │         │                         │              │  │
│  │  ┌──────┴─────────────────────────┴───────────┐  │  │
│  │  │           LanceDB 持久化层                  │  │  │
│  │  │   (向量 + 原文 + 元数据 + IVF_PQ 索引)      │  │  │
│  │  └───────────────────────────────────────────┘  │  │
│  └─────────────────────────────────────────────────┘  │
│                         │                              │
│              ┌──────────┴──────────┐                  │
│              │     Ollama (可选)    │                  │
│              │  /api/embed 嵌入替代 │                  │
│              └─────────────────────┘                  │
└──────────────────────────────────────────────────────┘
```

## 2. 分步实施策略

### 第一步：记忆管线

记忆管线嵌入现有对话流程，在 `ConversationStore::send_message` 中插入预处理和后处理环节。

#### 2.1 数据流

```
用户输入
  │
  ▼
意图判断（规则+启发式，内置零依赖）
  │
  ├── 不触发记忆 → 原文作为 prompt ──→ 发送到 AI API
  │                                        │
  └── 触发记忆 → Qwen3-Embedding 编码查询向量   │
                  │                        │
                  ▼                        │
            LanceDB 向量近邻搜索            │
                  │                        │
                  ▼                        │
            拼接记忆上下文到 prompt ────────┘
                                           │
                                           ▼
                                    AI API 流式回复
                                           │
                                           ▼
                                  回复内容向量化持久化
                                  (Qwen3-Embedding 编码
                                   → LanceDB 写入)
```

#### 2.2 新增模块

| 模块 | 路径 | 职责 |
|------|------|------|
| 嵌入器 | `src/services/embedder/mod.rs` | Qwen3-Embedding GGUF 推理封装（embellama），Ollama 可选扩展 |
| 意图分析 | `src/services/intent/mod.rs` | 规则+关键词启发式意图判断 |
| 向量存储 | `src/services/vector_store/mod.rs` | LanceDB 封装，向量写入/近邻搜索 |
| 全文索引 | `src/services/search/index.rs` | Tantivy 索引构建/查询 |
| 记忆管线 | `src/services/memory/mod.rs` | 编排意图判断→检索→组装→持久化 |

#### 2.3 嵌入器设计（`services/embedder`）

```rust
// src/services/embedder/mod.rs

pub trait Embedder: Send + Sync {
    fn encode(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>>;
    fn encode_one(&self, text: &str) -> anyhow::Result<Vec<f32>>;
    fn dimension(&self) -> usize;
    fn name(&self) -> &str;
}

// 内置 Qwen3-Embedding GGUF 实现（embellama 引擎）
pub struct Qwen3Embedder { /* embellama::EmbeddingEngine */ }

// Ollama /api/embed 可选实现（services/ollama/embed.rs）
pub struct OllamaEmbedder { /* reqwest Client + base_url */ }
```

**Qwen3Embedder 关键实现细节：**
- 依赖：`embellama`（llama.cpp Rust 绑定）、`llama-cpp-2`
- 模型文件：`qwen3-embedding-0.6b-q8_0.gguf`（~490MB，运行时下载到 `~/.xechat/models/`）
- 推理流程：分词 → llama_decode() → Last Token Pooling → L2 归一化 → 1024 维向量
- **Pooling 策略**：必须使用 **Last**（decoder 架构下 Mean pooling 会稀释语义）
- **关键参数**：n_batch=8192, n_seq_max=1, n_ubatch=512 → effective_max_tokens=8190
- 全局单例 `OnceLock<Arc<dyn Embedder>>`，应用启动时初始化

**OllamaEmbedder 可选扩展：**
- 通过 HTTP POST `/api/embed` 调用，支持 options.num_ctx / num_batch 参数
- 配置中可指定嵌入模型名（如 `qwen3-embedding:latest`）
- 切换时通过 `should_enable_ollama()` / `is_ollama_provider_selected()` 守卫防止误触发

#### 2.4 意图分析设计（`services/intent`）

```rust
// src/services/intent/mod.rs

#[derive(Debug, Clone)]
pub struct IntentResult {
    pub needs_memory: bool,
    pub confidence: f32,
    pub memory_query: String,
    pub time_hint: TimeRange,
    pub action: Action,
}

pub enum Action {
    DirectQuery,       // 直接发送原文
    SimpleContext,     // 附加最近3轮上下文
    MemoryRetrieve,    // 需要检索历史记忆
}

pub enum TimeRange {
    Any,
    RecentDays(u32),
    SpecificMonth(String),
}

pub struct BuiltinIntentAnalyzer { /* 预编译正则 */ }

impl BuiltinIntentAnalyzer {
    pub fn analyze(&self, input: &str, recent_context: &[Message]) -> IntentResult;
}
```

**触发规则：**
- 直接记忆触发词：之前/上次/说过/提过/记得/继续/帮我找/搜索/查找
- 引用型：你刚才/你之前/上文/前面提到
- 比较型：对比/比较/和之前/有什么不同
- 时间范围提取：最近/上周/昨天/2024年5月
- 跟进问题检测：当前输入与最近消息主题重叠度 > 30%

#### 2.5 向量存储设计（`services/vector_store`）

```rust
// src/services/vector_store/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,              // UUID
    pub conversation_id: String, // 对话 ID
    pub message_id: String,      // 消息 ID
    pub content: String,         // 原文片段
    pub role: String,            // "user" | "assistant"
    pub timestamp: DateTime<Utc>,
    pub embedding: Vec<f32>,     // 1024 维向量
}

pub trait VectorStore: Send + Sync {
    fn add(&self, entry: MemoryEntry) -> Result<()>;
    fn add_batch(&self, entries: Vec<MemoryEntry>) -> Result<()>;
    fn search(&self, query_vector: &[f32], top_k: usize) -> Result<Vec<SearchHit>>;
    fn delete_by_conversation(&self, conv_id: &str) -> Result<()>;
    fn delete_by_message(&self, msg_id: &str) -> Result<()>;
}

pub struct SearchHit {
    pub entry: MemoryEntry,
    pub score: f32,
}
```

**LanceDB 实现：**
- 依赖：`lancedb` crate
- 数据目录：`~/.xechat/vector_data/`
- 表结构：id, conversation_id, message_id, content, role, timestamp, vector(1024)
- 索引：IVF_PQ 向量索引（数据量 > 10000 条时自动构建）
- 元数据过滤：支持按 conversation_id、时间范围过滤

#### 2.6 记忆管线编排（`services/memory`）

```rust
// src/services/memory/mod.rs

pub struct MemoryPipeline {
    embedder: Arc<dyn Embedder>,
    intent_analyzer: BuiltinIntentAnalyzer,
    vector_store: Arc<dyn VectorStore>,
}

impl MemoryPipeline {
    /// 预处理：意图判断 + 记忆检索 + Prompt 组装
    pub fn preprocess(&self, user_input: &str, recent_messages: &[Message])
        -> PreprocessResult;

    /// 后处理：回复内容向量化持久化
    pub fn postprocess(&self, conv_id: &str, msg_id: &str, content: &str, role: &str)
        -> Result<()>;
}

pub struct PreprocessResult {
    pub enhanced_messages: Vec<ChatMessage>, // 可能注入了记忆上下文
    pub memory_used: bool,
}
```

**Prompt 组装策略：**
- 不触发记忆：原样传递用户消息
- 触发记忆：在用户消息前注入系统提示 + 检索到的记忆片段
  ```
  [系统] 以下是与用户问题相关的历史记忆：
  1. [2024-05-12] 用户之前讨论了 Rust 异步编程...
  2. [2024-06-01] 用户提到项目使用 Dioxus 框架...

  请结合以上记忆上下文回答用户的问题。
  ```

#### 2.7 集成到现有对话流程

修改 `ConversationStore::send_message`：

```rust
// 伪代码，展示集成点
pub async fn send_message(&mut self, content: String, config: XEChatConfig, ...) {
    // ... 现有校验逻辑 ...

    let pipeline = get_memory_pipeline(); // 全局单例

    // === 新增：预处理 ===
    let recent_msgs: Vec<Message> = self.selected_conversation()
        .map(|c| c.messages.clone())
        .unwrap_or_default();
    let preprocess_result = pipeline.preprocess(&content, &recent_msgs);

    // 使用增强后的消息列表替代原始消息
    let all_messages = if preprocess_result.memory_used {
        preprocess_result.enhanced_messages
    } else {
        // 现有的消息组装逻辑
        build_messages(...)
    };

    // ... 现有流式发送逻辑 ...

    // === 新增：后处理 ===
    // 在 StreamEvent::Complete 分支中
    pipeline.postprocess(&conv_id, &assistant_msg.id, &body, "assistant")?;
    // 同时索引用户消息
    pipeline.postprocess(&conv_id, &user_msg.id, &content, "user")?;
}
```

#### 2.8 新增 Cargo 依赖

```toml
# 向量数据库
lancedb = "0.30"

# 本地嵌入（llama.cpp Rust 绑定）
embellama = "0.10"
llama-cpp-2 = "0.1"

# 全文搜索（LanceDB 内置全文索引，无需 Tantivy）

# 错误处理
anyhow = "1"
```

---

### 第二步：搜索界面

#### 2.9 全文索引设计（`services/search`）

```rust
// src/services/search/index.rs

pub struct SearchIndex {
    writer: tantivy::IndexWriter,
    reader: tantivy::IndexReader,
    schema: tantivy::Schema,
}

// Tantivy Schema
// field          | type      | indexed | stored
// ---------------|-----------|---------|--------
// conversation_id| STR       | yes     | yes
// message_id     | STR       | yes     | yes
// content        | TEXT      | yes     | yes
// role           | STR       | yes     | yes
// timestamp      | I64       | yes     | yes
// title          | TEXT      | yes     | yes

pub struct SearchResult {
    pub conversation_id: String,
    pub message_id: String,
    pub title: String,
    pub content_snippet: String,  // 高亮摘要
    pub role: String,
    pub timestamp: DateTime<Utc>,
    pub score: f32,
    pub search_type: SearchType,  // FullText | Semantic | Hybrid
}

pub enum SearchType {
    FullText,
    Semantic,
    Hybrid,
}

impl SearchIndex {
    pub fn index_message(&self, conv_id: &str, msg_id: &str, title: &str,
                         content: &str, role: &str, timestamp: DateTime<Utc>) -> Result<()>;
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>>;
    pub fn delete_by_conversation(&self, conv_id: &str) -> Result<()>;
}
```

#### 2.10 混合搜索策略

```
用户搜索查询
      │
      ├── Tantivy 全文搜索 → BM25 排序结果
      │
      ├── Qwen3-Embedding 编码 → LanceDB 向量近邻搜索 → 余弦相似度排序结果
      │
      └── 融合排序（Reciprocal Rank Fusion）
            │
            ▼
      合并去重后的 Top-K 结果
```

**RRF 融合公式：**
```
score(d) = Σ 1/(k + rank_i(d))   其中 k=60
```

#### 2.11 搜索界面 UI

新增路由 `MainRoute::Search`，搜索界面采用两阶段布局：

**阶段一：搜索首页（无选中结果）**

搜索框居中，下方展示搜索结果列表：

```
┌─────────────────────────────────────────────┐
│                                             │
│          🔍 搜索框        [全文|语义|混合]    │
│                                             │
├─────────────────────────────────────────────┤
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │ 对话标题                    2024-06-15  │  │
│  │ ...关于Rust异步编程的讨论，提到了       │  │
│  │ tokio和async-trait的使用方法...         │  │
│  │ [全文匹配]                              │  │
│  └────────────────────────────────────────┘  │
│                                              │
│  ┌────────────────────────────────────────┐  │
│  │ 对话标题                    2024-05-20  │  │
│  │ ...之前讨论的Dioxus框架架构...          │  │
│  │ [语义相关 92%]                          │  │
│  └────────────────────────────────────────┘  │
│                                              │
└─────────────────────────────────────────────┘
```

**阶段二：选中结果后（左右并列）**

点击搜索结果后，左侧保留搜索结果列表，右侧展示对话内容预览：

```
┌────────────────────────┬────────────────────────────────┐
│  🔍 搜索框 [全文|语义]  │                                │
├────────────────────────┤     对话标题                    │
│                        │     2024-06-15                  │
│ ┌────────────────────┐ │                                │
│ │►对话标题  06-15    │ │  ┌──────────────────────────┐  │
│ │  Rust异步编程...   │ │  │ 👤 用户:                  │  │
│ │  [全文匹配]        │ │  │ 关于Rust异步编程...        │  │
│ └────────────────────┘ │  └──────────────────────────┘  │
│                        │  ┌──────────────────────────┐  │
│ ┌────────────────────┐ │  │ 🤖 助手:                  │  │
│ │ 对话标题  05-20    │ │  │ tokio和async-trait的...   │  │
│ │  Dioxus框架...     │ │  │ ← 高亮定位到此消息        │  │
│ │  [语义相关 92%]    │ │  └──────────────────────────┘  │
│ └────────────────────┘ │                                │
│                        │                                │
└────────────────────────┴────────────────────────────────┘
     搜索结果列表(40%)          对话预览(60%)
```

**交互流程：**
1. 用户输入搜索词 → 选择搜索模式（全文/语义/混合）→ 回车搜索
2. 搜索框下方展示搜索结果列表（每条：标题+内容预览+时间+类型标签）
3. 点击某条结果 → 布局切换为左右并列：左侧搜索列表，右侧对话预览
4. 右侧对话预览自动滚动定位到语义关联的消息位置并高亮
5. 点击左侧其他结果 → 右侧预览切换到对应对话
6. 点击搜索框重新搜索 → 右侧预览清空，回到全宽结果列表

#### 2.12 定位到消息

点击搜索结果后：
1. 右侧面板加载对应对话的消息列表
2. 传递 `highlight_message_id` 参数
3. `MessageList` 组件接收参数，滚动到指定消息并高亮
4. 不需要路由跳转，搜索界面内直接展示对话预览

```rust
// MainRoute 扩展
pub enum MainRoute {
    Welcome,
    Conversation(String),
    Settings,
    Search, // 新增
}

// SearchStore 扩展
pub struct SearchStore {
    // ... 现有字段 ...
    pub selected_result: Signal<Option<SearchResult>>,  // 当前选中的搜索结果
    pub preview_conversation_id: Signal<Option<String>>, // 预览的对话ID
    pub highlight_message_id: Signal<Option<String>>,   // 高亮的消息ID
}
```

#### 2.13 索引同步策略

- **实时索引**：每次消息完成（StreamEvent::Complete）时同步写入 Tantivy + LanceDB
- **启动重建**：应用启动时检查索引版本，必要时从对话文件全量重建
- **删除同步**：对话删除时同步清理 Tantivy + LanceDB 中的对应记录

---

## 3. 配置扩展

在 `XEChatConfig` 中新增记忆和搜索相关配置：

```toml
[memory]
enabled = true
embed_provider = "default"   # "default" (内置 Qwen3) | "ollama"
ollama_embed_model = ""     # Ollama 嵌入模型名，如 "qwen3-embedding:latest"
max_memory_results = 5      # 记忆检索返回的最大条数

[search]
default_mode = "hybrid"     # "fulltext" | "semantic" | "hybrid"
results_per_page = 20
```

---

## 4. 文件组织

新增模块结构：

```
src/
├── services/
│   ├── embedder/
│   │   ├── mod.rs           # Embedder trait + 全局单例
│   │   ├── qwen3.rs         # Qwen3-Embedding GGUF 嵌入器（默认）
│   │   └── manager.rs       # EmbedManager + 语义分块
│   ├── intent/
│   │   └── mod.rs           # BuiltinIntentAnalyzer
│   ├── vector_store/
│   │   ├── mod.rs           # VectorStore trait
│   │   └── lancedb_store.rs # LanceDB 实现
│   ├── search/
│   │   ├── mod.rs           # 全文搜索 + 语义搜索 + 混合搜索
│   │   └── hybrid.rs        # RRF 融合排序
│   ├── ollama/
│   │   └── embed.rs         # Ollama /api/embed 实现
│   └── memory/
│       └── mod.rs           # MemoryPipeline 编排
├── views/
│   └── search/
│       ├── mod.rs              # 搜索页面组件
│       ├── search_box.rs       # 搜索输入框
│       ├── search_results.rs   # 搜索结果列表
│       ├── conversation_preview.rs # 对话预览面板
│       └── recent_conversations.rs # 最近对话
├── stores/
│   └── conversation.rs    # ConversationStore（含 reinit_embedder / rebuild_vectors）
└── models/
    └── memory.rs          # TurnEntry, ChunkMeta, SearchResult 等数据模型
```

---

## 5. 错误处理与降级

| 场景 | 处理策略 |
|------|---------|
| Qwen3 模型文件不存在 | 自动下载到 `~/.xechat/models/`，下载中禁用语义搜索 |
| Qwen3 模型加载失败 | 日志警告，禁用语义搜索和记忆管线，仅保留全文搜索 |
| LanceDB 写入失败 | 返回 anyhow::Error（含诊断信息），不静默丢弃数据 |
| Ollama 不可用 | 不降级到内置模型（防止向量维度不一致），配置标注不可用 |
| LanceDB 索引损坏 | 从对话文件全量重建索引（force_rebuild=true） |
| 内存不足（GGUF 推理） | 限制 n_ubatch=512 防止溢出 |

---

## 6. 性能考量

- **Qwen3 推理延迟**：单条 ~100-500ms（CPU 推理，取决于文本长度和 token 数）
- **LanceDB 搜索**：IVF_PQ 索引，万级数据 < 10ms
- **LanceDB 全文搜索**：毫秒级，适合实时搜索
- **内存占用**：embellama + GGUF 模型 ~1-2GB RSS（取决于 n_batch 配置）
- **索引体积**：约为原始数据的 1.5-2 倍
- **effective_max_tokens**：8190（n_batch=8192, n_seq_max=1 时）

---

## 7. 实施顺序

### 第一步：记忆管线（子项目一）— 已完成 ✅

1. 添加 Cargo 依赖（embellama, llama-cpp-2, lancedb, anyhow）
2. 实现 `models/memory.rs` — TurnEntry / ChunkMeta 数据模型
3. 实现 `services/embedder` — Qwen3Embedder（embellama GGUF 推理）+ Embedder trait
4. 实现 `services/intent` — BuiltinIntentAnalyzer（规则引擎）
5. 实现 `services/vector_store` — LanceDB 存储（lancedb_store.rs）
6. 实现 `services/memory` — MemoryPipeline 编排
7. 集成到 `ConversationStore::send_message` — 预处理 + 后处理
8. 配置扩展 — memory 配置项（embed_provider, ollama_embed_model）
9. 测试 — 单元测试 + 集成测试

### 第二步：搜索界面（子项目二）— 已完成 ✅

1. 实现 `services/search` — LanceDB 全文索引 + 混合搜索（RRF）
2. 实现 `stores/search.rs` — 搜索状态管理
3. 实现 `views/search` — 搜索界面 UI（两阶段布局）
4. 扩展 `MainRoute` — Search 路由 + ConversationPreview
5. 实现 `views/search/conversation_preview.rs` — 对话预览面板（容器内精确滚动）
6. 索引同步 — 消息完成时实时索引
7. 测试 — 搜索功能测试
