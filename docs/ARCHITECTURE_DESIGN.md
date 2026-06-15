# XEChat 架构设计

## 1. 设计理念

XEChat 是一款基于 Dioxus 0.7 的桌面 AI 聊天客户端，采用五层架构实现 UI 与业务的完全解耦：

```
┌──────────────────────────────────────────────────────────┐
│  views/        视图层，页面组合 + 页面级组件               │
│                → 组合 components，消费 stores              │
│                → 包含页面专属子组件（chat_input 等）        │
├──────────────────────────────────────────────────────────┤
│  components/   通用展示组件（rsx! 布局）                   │
│                → 只接收 Props + 消费 Signal                │
│                → 通过 hooks 获取 store，不直接 new          │
├──────────────────────────────────────────────────────────┤
│  hooks/        粘合剂层                                   │
│                → use_app() / use_conversation() 等         │
│                → use_*_provider() 初始化 + 注入             │
│                → 封装 Context 获取，桥接 Store              │
├──────────────────────────────────────────────────────────┤
│  stores/       业务状态层                                 │
│                → 持有 Signal，定义业务规则，无 rsx!          │
│                → 通过 pending_send 等信号与 Layout 协作     │
├──────────────────────────────────────────────────────────┤
│  services/     副作用层（I/O）                             │
│                → HTTP、文件、DB，纯 async                   │
│                → 函数式风格或 trait + impl，不持有状态       │
│                → 全局单例用 OnceCell 管理                   │
├──────────────────────────────────────────────────────────┤
│  models/       数据模型（DTO）                             │
│                → 纯 struct/enum，零 UI/I/O 依赖             │
│                → Trait 签名在此层，实现在 services          │
└──────────────────────────────────────────────────────────┘

横切层:
  state.rs       MainRoute、ThemeMode、Toast 等全局状态类型
  icons.rs       SVG 图标重导出（dioxus-iconify / tabler）
  assets.rs      资源文件嵌入
  platform/      平台特定代码（macos, windows, linux）
  utils/         纯工具函数（html, markdown, paths, datetime）
  dioxus_style   自研作用域样式 crate（#[with_css] 宏 + SCSS 编译 + 类名哈希）
```

**核心原则**：

1. **分层严格隔离**：上层可引用下层，下层不可引用上层
2. **Signal 作为状态核心**：Store 持有，Component 消费
3. **Context 依赖注入**：通过 hooks 层注入，避免 Props Drilling
4. **Service 层无状态**：纯 async，返回 Result 或通过 channel 推送
5. **组件纯展示**：rsx! 布局 + 回调，不直接调用 API

---

## 2. 数据流

### 2.1 应用启动

```
main.rs
  → 配置窗口（标题、尺寸、透明标题栏、图标）
  → build.rs 编译全局 SCSS → global.css
  → launch(App)

App 组件
  → use_app_provider()      → 加载 config.toml + 同步 theme/language
  → use_conversation_provider() → 加载对话列表 + 初始化后端（Qwen3-Embedding + LanceDB）
  → use_ui_provider()
  → use_search_provider()
  → 渲染 Layout + Notification + RenameModal + DeleteModal
```

### 2.2 路由与导航

```
Layout 组件
  → 读取 app_store.main_route
  → match route:
      MainRoute::Welcome       → Welcome（ChatInput 居中）
      MainRoute::Conversation(id) → ConversationView
      MainRoute::Settings      → SettingsView
      MainRoute::Search        → SearchView

新建对话:
  conv_store.current_conversation_id.set(None)
  app_store.navigate_to(MainRoute::Welcome)

发送消息创建对话:
  conv_store.create_temporary_conversation(title)
  app_store.navigate_to(MainRoute::Conversation(conv_id))
```

### 2.3 发送消息（关键数据流）

```
用户在 ChatInput 中输入并提交
  → 若无当前对话：create_temporary_conversation() + navigate_to()
  → conv_store.pending_send.set(Some((content, config)))  ← 关键：信号传递

Layout 组件（永不 unmount）
  → 读取 pending_send
  → conv_store.pending_send.set(None)
  → spawn(async move {
      conv_store.send_message(content, config, toast_callback).await
    })

ConversationStore.send_message()
  → 检查 current_conversation_id
  → is_streaming = true
  → 追加用户消息到对话
  → 记忆管线预处理（embed + vector search）
  → 上下文压缩（compress_messages）
  → tokio::spawn: send_message() → Provider 路由
  → tokio::select! 循环:
      CancellationToken → 截断保存
      StreamEvent::Chunk → streaming_content 追加
      StreamEvent::ReasoningChunk → streaming_reasoning 追加
      StreamEvent::Complete → 保存助手消息 + 记忆后处理 + 标题解析
      StreamEvent::Error → 保存失败消息 + Toast 通知
```

**为什么用 pending_send？**

Dioxus 的 `spawn` 绑定组件 scope，组件 unmount 时异步任务被取消。Welcome 页 ChatInput 发送消息后 `navigate_to` 导致组件 unmount，`spawn` 的任务被取消。而 `tokio::spawn` 不可用（`ConversationStore` 包含 `Signal`，非 `Send`）。通过 `pending_send` 信号将请求传递给 Layout（永不 unmount），在 Layout 的 scope 中安全 spawn。

---

## 3. AI Provider 架构

```
services/ai/
├── mod.rs              # send_message() 统一入口
├── streaming.rs        # SSE/NDJSON 解析 + token 估算 + 错误提取
└── providers/
    ├── mod.rs          # Provider 注册
    ├── deepseek.rs     # DeepSeek（SSE + reasoning_content）
    ├── openai.rs       # OpenAI（SSE 具名事件）
    ├── ollama.rs       # Ollama（NDJSON 流式）
    └── openai_compatible.rs  # 通用 OpenAI 兼容协议

路由规则:
  "deepseek" → DeepSeekProvider
  "openai"   → OpenAiProvider
  "ollama"   → OllamaProvider
  其他       → OpenAiCompatibleProvider
```

所有 Provider 实现 `AiProvider` trait，通过 `mpsc::UnboundedSender<StreamEvent>` 推送流式结果。

---

## 4. 记忆管线架构

```
用户消息 → MemoryPipeline.preprocess()
  → BuiltinIntentAnalyzer.analyze()  → 判断是否需要检索记忆
  → Embedder.encode_query()          → 文本向量化
  → VectorStore.search_turns()       → LanceDB 向量检索
  → 返回增强的 system message（记忆上下文）

助手回复完成 → MemoryPipeline.postprocess()
  → 配对用户消息 + 助手回复
  → Embedder.encode_passage()        → 轮次文本向量化
  → 语义分块（短文本整条编码，长文本分块编码）
  → VectorStore.add_turn()           → 写入 LanceDB
```

**组件关系**：

| 组件 | 实现 | 存储 |
|------|------|------|
| Embedder | Qwen3-Embedding GGUF（embellama）或 Ollama | 全局单例 |
| IntentAnalyzer | 正则匹配（BuiltinIntentAnalyzer） | 无状态 |
| VectorStore | LanceDB（lancedb_store） | 全局单例 |
| MemoryPipeline | 组合以上三者 | 全局单例 |

---

## 5. 搜索架构

```
SearchView
├── 空查询 → 最近对话列表（LanceDB 分页加载）
└── 有查询 → 搜索结果
    ├── fulltext_search()  → LanceDB 标量过滤
    └── semantic_search()  → 向量检索 + LanceDB 过滤
    └── hybrid             → 混合搜索（合并 + 去重 + 排序）

选中结果 → 右侧对话预览面板
```

---

## 6. 样式架构

### 双通道注入

| 通道 | 文件 | 加载方式 | 作用域 |
|------|------|---------|--------|
| 全局 | `tokens.scss`, `theme.scss`, `reset.scss`, `materials.scss`, `keyframes.scss`, `utilities.scss`, `markdown.scss` | `build.rs` 编译为 `global.css`，`main.rs` 注入 `<head>` | 全局 |
| 组件 | `styles/components/*.scss`, `styles/views/*.scss` | dioxus_style `#[with_css]` 宏 | 组件级（类名哈希） |

### dioxus_style 工作流

```
#[with_css(css, "styles/components/conversation.scss")]
→ 编译时: grass 编译 SCSS → CSS
→ 编译时: 提取类名 → 哈希（如 .conv-input → .a3f2b1）
→ 运行时: 注入 <style> 到 <head>
→ Rust 代码: css::conv_input → "a3f2b1"
```

**关键约束**：JS 选择器不能使用 dioxus_style 管理的 CSS 类名（会被哈希），必须改用 `data-*` 属性。

---

## 7. Store 架构

| Store | Signal 列表 | 职责 |
|-------|------------|------|
| AppStore | config, theme_mode, language, timezone, main_route | 全局配置、主题、路由 |
| ConversationStore | conversations, current_conversation_id, streaming_content, streaming_reasoning, is_streaming, pending_send, cancel_token, stream_task, message_pagination | 对话列表、消息收发、流式状态 |
| UIStore | show_config_modal, show_rename_modal, show_delete_modal, open_menu_id, open_header_menu, active_toast, menu_position | 弹窗、Toast、菜单 |
| SearchStore | query, results, search_type, is_searching, selected_result, recent_items, recent_page | 搜索状态 |

所有 Store 通过 `use_*_provider()` 在 App 组件中初始化并注入 Context，子组件通过 `use_*()` 获取。

---

## 8. 技术选型

| 层级 | 技术 | 说明 |
|------|------|------|
| UI 框架 | Dioxus 0.7 (Desktop) | RSX 声明式，Signal 响应式 |
| 状态管理 | Signal + Context | Dioxus 原生 |
| 样式 | SCSS + dioxus_style | fork 改良的作用域样式 crate（with_css 宏） |
| HTTP 客户端 | reqwest 0.11 | JSON + SSE 流式 |
| 异步运行时 | tokio | 全功能 |
| 向量数据库 | LanceDB | 对话持久化 + 向量检索 |
| 本地嵌入 | embellama (Qwen3-Embedding GGUF) | qwen3-embedding-0.6b |
| Markdown | comrak 0.52 | 含 syntect 代码高亮 |
| 数学公式 | katex 0.4 | 行内渲染 |
| 图表 | mermaid-rs-renderer | Mermaid 图表 |
| 序列化 | serde + serde_json | TOML/JSON |
| 配置格式 | TOML | toml crate |
| 国际化 | rust-i18n | zh-CN / en |
| ID 生成 | uuid v4 | 对话和消息 ID |
