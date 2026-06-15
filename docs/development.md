# XEChat 开发文档

## 项目结构

```
xechat/
├── Cargo.toml              # workspace: xechat + dioxus-style crates
├── Dioxus.toml             # 桌面打包配置
├── build.rs                # SCSS → CSS 编译（grass）+ KaTeX 注入
├── assets/                 # 静态资源（icon.png, katex/）
├── locales/                # i18n 翻译文件
│   ├── zh-CN.yml
│   └── en.yml
├── crates/                 # 本地 workspace crates
│   ├── dioxus-style/       # dioxus_style 运行时 CSS 注入
│   └── dioxus-style-macro/ # dioxus_style #[with_css] 过程宏
├── tests/                  # 集成测试
└── src/
    ├── main.rs             # 桌面入口
    ├── lib.rs              # 模块声明
    ├── app.rs              # App 根组件
    ├── state.rs            # MainRoute, ThemeMode, Toast
    ├── models/             # 数据模型层
    ├── services/           # 副作用层
    ├── stores/             # 业务状态层
    ├── hooks/              # 粘合剂层
    ├── components/         # 通用展示组件
    ├── views/              # 视图层
    ├── utils/              # 工具函数
    ├── styles/             # SCSS 样式
    ├── icons/              # SVG 图标
    └── platform/           # 平台特定代码
```

## 开发环境

### 前置要求

- Rust 1.85+（edition 2024）
- macOS: Xcode Command Line Tools
- Linux: `libwebkit2gtk-4.1-dev` 等 GTK/WebKit 依赖
- Windows: WebView2 Runtime

### 开发命令

```bash
# 开发运行
cargo run

# 编译检查（快速）
cargo check

# Release 构建
cargo build --release

# 运行测试
cargo test

# 运行单个测试
cargo test --test services_ai_streaming_test

# 文档生成
cargo doc --no-deps --open
```

## 技术栈

| 层级 | 技术 | 版本 |
|------|------|------|
| UI 框架 | Dioxus | 0.7 (Desktop) |
| HTTP 客户端 | reqwest | 0.11 |
| 异步运行时 | tokio | 1.x |
| 向量数据库 | LanceDB | 0.30 |
| 本地嵌入 | embellama | 0.10 |
| Markdown | comrak | 0.52 |
| 数学公式 | katex | 0.4 |
| 序列化 | serde | 1.x |
| 配置格式 | TOML | 0.8 |
| 国际化 | rust-i18n | 3.x |
| SCSS 编译 | grass | 0.13 (build.rs) |

## 组件树

```
App
├── div[data-theme]
│   ├── Layout
│   │   ├── button#__xechat_new_chat_btn  (隐藏，供全局快捷键)
│   │   ├── div.titlebar                   (拖拽 + 双击最大化)
│   │   └── div.app-body
│   │       ├── Sidebar
│   │       │   ├── SidebarHeader (品牌 + 新建对话按钮)
│   │       │   ├── ConversationItem × N  (滚动加载)
│   │       │   └── SidebarFooter (设置按钮)
│   │       └── div.main-content
│   │           ├── Welcome                (ChatInput 居中)
│   │           ├── ConversationView       (对话页)
│   │           │   ├── ConversationHeader (标题)
│   │           │   ├── MessageList        (消息 + 流式)
│   │           │   └── ChatInput          (输入框 + 模型选择器)
│   │           ├── SettingsView           (设置页)
│   │           └── SearchView             (搜索页)
│   ├── Notification       (Toast 通知)
│   ├── RenameModal        (重命名对话框)
│   └── DeleteModal        (删除确认对话框)
```

## 状态管理

应用状态通过 Dioxus `Signal` 管理，通过 hooks 层的 `use_*_provider()` 注入 Context。

### AppStore

| Signal | 类型 | 说明 |
|--------|------|------|
| `config` | `Signal<Option<XEChatConfig>>` | 应用配置 |
| `theme_mode` | `Signal<ThemeMode>` | 主题模式 |
| `language` | `Signal<Language>` | 界面语言 |
| `timezone` | `Signal<String>` | 时区 |
| `main_route` | `Signal<MainRoute>` | 当前路由 |

### ConversationStore

| Signal | 类型 | 说明 |
|--------|------|------|
| `conversations` | `Signal<Vec<Conversation>>` | 对话列表 |
| `current_conversation_id` | `Signal<Option<String>>` | 当前对话 ID |
| `streaming_content` | `Signal<String>` | 流式输出内容 |
| `streaming_reasoning` | `Signal<String>` | 流式推理内容 |
| `is_streaming` | `Signal<bool>` | 是否正在流式传输 |
| `pending_send` | `Signal<Option<(String, XEChatConfig)>>` | 待发送消息 |
| `cancel_token` | `Signal<Option<CancellationToken>>` | 取消令牌 |
| `message_pagination` | `Signal<MessagePagination>` | 消息分页 |

### UIStore

| Signal | 类型 | 说明 |
|--------|------|------|
| `show_config_modal` | `Signal<bool>` | 设置弹窗 |
| `show_rename_modal` | `Signal<Option<String>>` | 重命名弹窗 |
| `show_delete_modal` | `Signal<Option<String>>` | 删除弹窗 |
| `open_menu_id` | `Signal<Option<String>>` | 右键菜单 |
| `active_toast` | `Signal<Option<Toast>>` | Toast 通知 |

### SearchStore

| Signal | 类型 | 说明 |
|--------|------|------|
| `query` | `Signal<String>` | 搜索关键词 |
| `results` | `Signal<Vec<SearchResult>>` | 搜索结果 |
| `search_type` | `Signal<SearchType>` | 搜索类型 |
| `is_searching` | `Signal<bool>` | 是否搜索中 |
| `selected_result` | `Signal<Option<SearchResult>>` | 选中结果 |
| `recent_items` | `Signal<Vec<ConversationSummary>>` | 最近对话 |

## 数据流

### 应用启动

```
App → use_app_provider()
    → load_config() → 同步 theme_mode / language
    → use_conversation_provider()
    → load_conversations() + init_backend()（Qwen3-Embedding + LanceDB）
```

### 新建对话

```
用户点击新建 / Cmd+K
  → conv_store.current_conversation_id.set(None)
  → app_store.navigate_to(MainRoute::Welcome)
```

### 发送消息

```
ChatInput 提交
  → 若无对话: create_temporary_conversation() + navigate_to()
  → pending_send.set(Some((content, config)))

Layout 消费 pending_send
  → spawn(async { send_message().await })

send_message()
  → 追加用户消息 → 记忆预处理 → 上下文压缩
  → tokio::spawn: Provider 流式请求
  → select! 循环: Chunk / Reasoning / Complete / Error / Cancel
```

### 保存配置

```
SettingsView 修改
  → app_store.update_config(|c| { ... })
  → save_config() → 持久化到 config.toml
```

## 国际化

使用 `rust-i18n` crate，翻译文件位于 `locales/zh-CN.yml` 和 `locales/en.yml`。

```rust
// 使用方式
let text = t!("chat.input-placeholder");
let text = t!("settings.openai-compatible");
```

键名层级：`{模块}.{功能}`，如 `chat.input-placeholder`、`settings.title`。

新增 UI 文本时必须同步更新两个语言文件。

## 样式开发

### 全局样式

`build.rs` 将以下 SCSS 编译为 `global.css`，在 `main.rs` 中注入 `<head>`：

- `tokens.scss` — CSS 变量（设计令牌）
- `theme.scss` — 主题变量（dark/light）
- `reset.scss` — 样式重置
- `materials.scss` — 通用材质
- `keyframes.scss` — 动画关键帧
- `utilities.scss` — 工具类
- `markdown.scss` — Markdown 渲染样式

### 组件样式

使用 dioxus_style 的 `#[with_css]` 宏按需注入，类名自动哈希：

```rust
#[with_css(css, "styles/components/conversation.scss")]
pub fn ChatInput() -> Element {
    rsx! {
        div { class: "{css::conv_input_container}", ... }
    }
}
```

**注意**：JS 选择器不能使用 dioxus_style 管理的 CSS 类名（会被哈希），改用 `data-*` 属性。

## 测试

测试按层分目录组织在 `tests/` 下：

```
tests/
├── common/mod.rs
├── models/          # ai, config, conversation, error, i18n, memory, message
├── services/        # ai/streaming, conversation, conversation_store, embedder, intent, memory, ollama, vector_store
├── stores/          # app, conversation, ui
├── components/      # conversation_screen, input
└── utils/           # html, markdown, paths
```

```bash
# 运行所有测试
cargo test

# 运行指定层测试
cargo test --test models_ai_test
cargo test --test services_ai_streaming_test
```

## 嵌入架构

### 模型选择

XEChat 支持两种嵌入模式，通过 `config.preferences.embed_provider` 切换：

| 模式 | 值 | 说明 |
|------|-----|------|
| 内置模式 | `"default"` | 使用本地 GGUF 文件 + embellama 引擎（默认） |
| Ollama 模式 | `"ollama"` | 通过 HTTP 调用 Ollama `/api/embed` 端点 |

### 内置嵌入模型：Qwen3-Embedding

当前内置模型为 **qwen3-embedding-0.6b-q8_0.gguf**（1024 维），基于 Qwen2 decoder 架构。

**关键配置要点**：

```rust
// src/services/embedder/qwen3.rs

// 1. Pooling 策略必须为 Last（decoder 嵌入模型标准做法）
//    Mean pooling 在 causal attention 下会稀释早期 token 的弱表示
.with_pooling_strategy(PoolingStrategy::Last)

// 2. 显式配置 llama 参数以支持长文本输入
//    decoder 默认 n_batch=2048, n_seq_max=2 → effective_max=1022（过小）
.with_n_batch(8192)      // 可用上下文窗口
.with_n_seq_max(1)       // 嵌入只需单序列
.with_n_ubatch(512)      // 必须显式设置，防止自动推导溢出 SIGSEGV
```

**effective max tokens 公式**：`n_batch / n_seq_max - 2`

| 配置 | n_batch | n_seq_max | effective_max |
|------|---------|-----------|---------------|
| decoder 默认 | 2048 | 2 | **1022** |
| 当前推荐 | 8192 | 1 | **8190** |

### Decoder vs Encoder 判定

embellama 通过 GGUF 元数据中的架构字符串判断模型类型：

- **Decoder**（`is_decoder=true`）：调用 `ctx.decode()` → 自回归解码 + causal attention mask → **需要 Last pooling**
- **Encoder**（`is_decoder=false`）：调用 `ctx.encode()` → 双向编码 + bidirectional attention → **使用 Mean pooling**

**注意**：qwen3-embedding 架构名含 `"qwen"`，被 embellama 归类为 decoder。**不要强制改为 encoder**——`ctx.encode()` 对该模型会导致 SIGSEGV。正确做法是保持 decoder 模式 + Last pooling。

### Cargo Patch 机制

当第三方 crate（如 embellama）的行为不符合需求时，使用 `[patch.crates-io]` 本地修改源码，无需 fork 整个仓库：

```toml
# Cargo.toml
[patch.crates-io]
embellama = { path = ".cargo/patch/embellama" }
```

```
.cargo/patch/embellama/     ← 完整的 embellama 源码副本
└── src/
    ├── gguf.rs             ← 架构判定逻辑
    ├── model.rs            ← 参数推导、flash attention 等
    └── ...
```

**使用规范**：
- patch 目录仅包含需要修改的文件差异，其余与原版一致
- 每次 patch 修改需在 commit message 中说明
- 避免过度 patch——优先在项目代码层解决（如 qwen3.rs 中的参数配置）

### Ollama API Options

Ollama 模式下通过 HTTP options 传递参数：

```rust
// src/services/ollama/embed.rs
.json(&serde_json::json!({
    "model": self.model,
    "input": texts,
    "options": {
        "num_ctx": self.context_window,   // 上下文长度
        "num_batch": self.context_window,  // 批处理大小
    }
}))
```

### 嵌入器切换防御规则

切换嵌入提供商时存在"配置不完整导致误触发重建"的风险。以下场景必须防护：

| 场景 | embed_provider | embed_model | 正确行为 |
|------|---------------|-------------|---------|
| 用户选 ollama provider | `"ollama"` | `""` | **不触发 reinit**，等用户选模型 |
| 用户选具体模型 | `"ollama"` | `"qwen3-..."` | 触发 reinit |
| 应用启动（配置完整） | `"ollama"` | `"qwen3-...""` | 正常初始化 Ollama |
| 应用启动（未配完） | `"ollama"` | `""` | **跳过 init**，不 fallback 内置 |
| 心跳检测（未配完） | `"ollama"` | `""` | 返回 false，不误报 ready |
| 切回内置模式 | `"default"` | `""` | 初始化 Qwen3 + rebuild |

**关键函数**：
- `should_enable_ollama(config)` — ollama 已完整配置（provider + model 都有值）
- `is_ollama_provider_selected(config)` — 仅检查 provider（用于启动/心跳守卫）
- `init_embedder()` — 三级判断：ollama 完整 → ollama 未配完(跳过) → 内置模式

## 向量存储与索引

### LanceDB 表结构

```
turns.lance
├── id: string (primary key)
├── conversation_id: string
├── role: string (user/assistant)
├── content: string
├── timestamp: datetime
├── vector: fixed_size_list<float32>[dim]  (向量列)
└── metadata: string (JSON)
```

### 向量索引策略

LanceDB 使用 IVF_PQ 索引加速 ANN 搜索。索引构建遵循以下规则：

| 条件 | 行为 |
|------|------|
| 行数 < `MIN_INDEX_ROWS`(10000) | 不构建索引（全量扫描更快） |
| 行数 ≥ MIN_INDEX_ROWS 且无索引 | 构建 IVF_PQ 索引 |
| 行数增长 >50% 或距上次 >24h | 触发增量重建 |
| `force_rebuild=true` | 删除旧表重建（忽略已存在的 turn） |

### 维度校验

向量写入前进行维度一致性检查：

- **正常流程**：`resolve_vector_dim()` 从全局 embedder 获取维度
- **维度不匹配**：返回 `anyhow::Error`（含 chunk 索引、期望/实际维度、Turn ID），不再静默丢弃数据
- **embedder 未就绪**：fallback 到 `DEFAULT_VECTOR_DIM`(1024)，输出 WARNING 日志

### Rebuild Vectors 三场景

| 触发点 | force_rebuild | 用途 |
|--------|--------------|------|
| `init_backend()` | `false` | 应用启动，断点续传（跳过已有 turn） |
| `reinit_embedder()` | `true` | 模型变更后，全量重建 |
| `rebuild_vectors()` | `true` | 用户手动触发，强制覆盖 |

批量重建完成后自动触发一次 `maybe_rebuild_vector_index()` 确保索引就绪。
