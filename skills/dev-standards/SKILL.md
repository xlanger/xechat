---
name: "dev-standards"
description: "Use when working on any XEChat project task — including bug fixes, refactoring, new features, feature migration, style changes, or config updates. Enforces five-layer architecture, naming conventions, file organization, coding style, and documentation annotations."
---

# XEChat 开发规范

## 铁律

```
所有需求都适用本规范。无例外。
```

**违反规范的字面意思就是违反规范的精神。**

"只是修个 BUG" ≠ 豁免文档注解
"只是改个变量名" ≠ 豁免命名规范
"只是加一行代码" ≠ 豁免分层架构
"这个改动太小" ≠ 跳过检查清单
"只改了样式文件" ≠ 豁免命名规范

## 1. 五层架构

严格遵循自上而下的依赖方向，上层可引用下层，反之不可：

```
┌──────────────────────────────────────────────────┐
│  views/        视图层，页面组合 + 页面级组件        │
│                → 组合 components，消费 stores       │
│                → 包含页面专属子组件（chat_input 等）  │
├──────────────────────────────────────────────────┤
│  components/   通用展示组件（rsx! 布局）            │
│                → 只接收 Props + 消费 Signal         │
│                → 通过 hooks 获取 store，不直接 new   │
├──────────────────────────────────────────────────┤
│  hooks/        粘合剂层                            │
│                → use_app() / use_conversation() 等  │
│                → use_*_provider() 初始化 + 注入      │
│                → 封装 Context 获取，桥接 Store       │
├──────────────────────────────────────────────────┤
│  stores/       业务状态层                          │
│                → 持有 Signal，定义业务方法           │
│                → 无 rsx!，不直接操作 DOM             │
│                → 调用 services 完成 I/O             │
│                → 通过 pending_send 等信号与 Layout 协作 │
├──────────────────────────────────────────────────┤
│  services/     副作用层（I/O）                      │
│                → HTTP、文件、DB，纯 async            │
│                → 函数式风格或 trait + impl，不持有状态 │
│                → 返回 Result 或使用 channel         │
├──────────────────────────────────────────────────┤
│  models/       数据模型（DTO）                      │
│                → 纯 struct/enum，零 UI/I/O 依赖      │
│                → #[derive(Clone, Serialize, ...)]   │
│                → Trait 签名在此层，实现在 services    │
└──────────────────────────────────────────────────┘

横切层:
  state.rs       MainRoute、ThemeMode、Toast 等全局状态类型
  icons.rs       SVG 图标重导出（dioxus-iconify）
  assets.rs      资源文件嵌入
  platform/      平台特定代码（macos, windows, linux）
  utils/         纯工具函数（html, markdown, paths, datetime）
```

## 2. 目录结构

```
xechat/
├── Cargo.toml              # workspace: xechat + dioxus-style crates
├── Dioxus.toml             # 桌面打包配置（identifier, icon, resources）
├── build.rs                # SCSS → CSS 编译（grass crate）+ KaTeX 注入
├── assets/                 # 静态资源（icon.png, katex/）
├── locales/                # i18n 翻译文件
│   ├── zh-CN.yml
│   └── en.yml
├── crates/                 # 本地 workspace crates
│   ├── dioxus-style/       # 运行时 CSS 注入
│   └── dioxus-style-macro/ # dioxus_style #[with_css] 过程宏 + SCSS 编译 + 类名哈希
├── tests/                  # 集成测试（按层分目录）
│   ├── models/
│   ├── services/
│   ├── stores/
│   ├── components/
│   ├── utils/
│   └── common/mod.rs
│
└── src/
    ├── main.rs             # 桌面入口：窗口配置 + Dioxus launch
    ├── lib.rs              # 模块声明 + pub use models::*
    ├── app.rs              # App 根组件：provider 注入 + 主题 + 模态框
    ├── state.rs            # MainRoute, ThemeMode, Toast, ToastKind
    ├── assets.rs           # 资源嵌入
    ├── icons/              # SVG 图标（tabler.rs）
    │
    ├── models/             # 纯数据模型层
    │   ├── mod.rs          # pub mod + pub use
    │   ├── ai.rs           # ChatMessage, StreamEvent, SendMessageParams, AiProvider trait
    │   ├── config.rs       # XEChatConfig, ModelProvider, ModelConfig, MemoryConfig, PreferencesConfig
    │   ├── conversation.rs # Conversation
    │   ├── error.rs        # AppError, AuthFailReason
    │   ├── i18n.rs         # Language, set_language()
    │   ├── memory.rs       # SearchHit, TurnEntry, ChunkMeta, IntentResult, SearchResult
    │   └── message.rs      # Message, MessageRole, MessageStatus
    │
    ├── services/           # 副作用层（I/O）
    │   ├── mod.rs
    │   ├── config.rs       # 配置文件读写（TOML）
    │   ├── paths.rs        # 应用数据路径管理
    │   ├── conversation.rs # 对话业务逻辑（CRUD 委托 conversation_store）
    │   ├── conversation_store/  # LanceDB 对话持久化
    │   │   └── mod.rs      # 对话 CRUD、消息追加、全文搜索、语义搜索
    │   ├── ai/             # AI 交互模块
    │   │   ├── mod.rs      # send_message() 统一入口 + Provider 路由
    │   │   ├── streaming.rs # SSE/NDJSON 解析 + token 压缩 + 错误提取
    │   │   └── providers/
    │   │       ├── mod.rs  # Provider 注册
    │   │       ├── deepseek.rs       # DeepSeek（SSE + reasoning_content）
    │   │       ├── openai.rs         # OpenAI（SSE 具名事件）
    │   │       ├── ollama.rs         # Ollama（NDJSON 流式）
    │   │       └── openai_compatible.rs  # 通用 OpenAI 兼容
    │   ├── embedder/       # 文本向量化抽象层
    │   │   ├── mod.rs      # Embedder trait + 全局单例
    │   │   ├── e5.rs       # E5 GGUF 本地嵌入器（embellama）
    │   │   └── manager.rs  # EmbedManager + 语义分块
    │   ├── intent/         # 用户意图识别
    │   │   └── mod.rs      # BuiltinIntentAnalyzer（正则匹配）
    │   ├── vector_store/   # 向量存储抽象层
    │   │   ├── mod.rs      # VectorStore trait
    │   │   └── lancedb_store.rs  # LanceDB 实现
    │   ├── memory/         # 记忆管线
    │   │   └── mod.rs      # MemoryPipeline（preprocess + postprocess）
    │   ├── ollama/         # Ollama 服务集成
    │   │   ├── mod.rs      # OllamaStatus, OllamaConfig
    │   │   ├── probe.rs    # 服务探测 + 模型分类
    │   │   └── embed.rs    # Ollama 嵌入器实现
    │   └── search/         # 搜索服务
    │       ├── mod.rs      # fulltext_search + semantic_search
    │       └── hybrid.rs   # 混合搜索
    │
    ├── stores/             # 业务状态层
    │   ├── mod.rs
    │   ├── app.rs          # AppStore（config, theme_mode, language, timezone, main_route）
    │   ├── conversation.rs # ConversationStore（conversations, streaming, pending_send）
    │   ├── ui.rs           # UIStore（modals, toast, menus）
    │   └── search.rs       # SearchStore（query, results, recent_items）
    │
    ├── hooks/              # 粘合剂层
    │   ├── mod.rs
    │   ├── use_app.rs      # use_app() / use_app_provider()
    │   ├── use_conversation.rs  # use_conversation() / use_conversation_provider()
    │   ├── use_ui.rs       # use_ui() / use_ui_provider()
    │   └── use_search.rs   # use_search() / use_search_provider()
    │
    ├── components/         # 通用展示组件
    │   ├── mod.rs
    │   ├── sidebar.rs      # 侧边栏（对话列表 + 滚动加载）
    │   ├── sidebar_header.rs  # 侧边栏头部（新建对话按钮）
    │   ├── sidebar_footer.rs  # 侧边栏底部
    │   ├── conversation_item.rs  # 对话列表项
    │   ├── notification.rs # Toast 通知
    │   ├── custom_select.rs  # 自定义下拉选择器
    │   ├── markdown.rs     # Markdown 渲染（含代码高亮、KaTeX、Mermaid）
    │   ├── input/          # 通用输入组件
    │   │   ├── mod.rs
    │   │   └── component.rs
    │   ├── collapse/       # 折叠面板
    │   │   ├── mod.rs
    │   │   └── component.rs
    │   └── modals/         # 模态框
    │       ├── mod.rs
    │       ├── modal.rs    # 通用模态框容器
    │       ├── rename.rs   # 重命名对话框
    │       └── delete.rs   # 删除确认对话框
    │
    ├── views/              # 视图层
    │   ├── mod.rs
    │   ├── layout.rs       # 全局布局（Sidebar + MainContent + pending_send 处理）
    │   ├── welcome.rs      # 欢迎页（ChatInput 居中）
    │   ├── conversation/   # 对话视图
    │   │   ├── mod.rs      # ConversationView 组合
    │   │   ├── header.rs   # 对话头部（标题）
    │   │   ├── message_list.rs   # 消息列表
    │   │   ├── message_bubble.rs # 消息气泡
    │   │   └── chat_input.rs     # 对话输入框（含内联模型选择器）
    │   ├── settings/       # 设置视图
    │   │   ├── mod.rs      # SettingsView 组合
    │   │   ├── general_section.rs    # 通用设置
    │   │   ├── provider_section.rs   # 模型提供商配置
    │   │   ├── memory_section.rs     # 记忆设置
    │   │   └── ollama_section.rs     # Ollama 配置
    │   └── search/         # 搜索视图
    │       ├── mod.rs      # SearchView 组合
    │       ├── search_box.rs           # 搜索输入框
    │       ├── search_results.rs       # 搜索结果列表
    │       ├── conversation_preview.rs # 对话预览
    │       └── recent_conversations.rs # 最近对话
    │
    ├── utils/              # 工具函数
    │   ├── mod.rs
    │   ├── html.rs         # HTML 转义
    │   ├── markdown.rs     # Markdown 渲染辅助
    │   ├── paths.rs        # 路径工具
    │   └── datetime.rs     # 日期时间格式化
    │
    ├── styles/             # SCSS 样式
    │   ├── tokens.scss     # CSS 变量（设计令牌）
    │   ├── theme.scss      # 主题变量（dark/light）
    │   ├── reset.scss      # 样式重置
    │   ├── materials.scss  # 通用材质样式
    │   ├── keyframes.scss  # 动画关键帧
    │   ├── utilities.scss  # 工具类
    │   ├── markdown.scss   # Markdown 渲染样式
    │   ├── mixins.scss     # SCSS 混入
    │   ├── components/     # 组件样式
    │   │   ├── conversation.scss  # 对话 + 输入框 + 模型选择器
    │   │   ├── sidebar.scss
    │   │   ├── settings.scss
    │   │   ├── notification.scss
    │   │   ├── custom_select.scss
    │   │   ├── input.scss
    │   │   ├── collapse.scss
    │   │   ├── modals/
    │   │   │   └── modal.scss
    │   │   ├── main_content.scss
    │   │   └── welcome.scss
    │   └── views/          # 视图样式
    │       ├── layout.scss
    │       ├── conversation.scss
    │       └── search.scss
    │
    └── platform/           # 平台特定代码
        ├── mod.rs
        ├── macos.rs
        ├── windows.rs
        └── linux.rs
```

## 3. 各层编码规范

### 3.1 Models

**规则**:

- 不引入 `dioxus` / `Signal`
- 不引入 async I/O 类型（`reqwest::Response`, `futures_util::StreamExt` 等）
- 可引入 `serde`、`chrono`、`uuid`
- Trait 定义放在 models 层（仅类型签名），实现在 services 层
- mod.rs 中 `pub use` 所有公共类型

### 3.2 Services

**规则**:

- 函数式风格或 trait + impl，不持有业务状态
- 参数明确，使用 channel (`mpsc`) 传递流式结果
- 引用 `crate::models::*` 作为数据类型
- 错误处理：通过 `StreamEvent::Error` 或 `anyhow::Result` 传递
- 全局单例用 `OnceCell` 管理（embedder、pipeline、conversation_store）
- 复杂 I/O 拆分为子模块（如 `ai/providers/`、`ollama/`）

### 3.3 Stores

**规则**:

- 持有 `Signal<T>` 响应式数据
- `new()` 初始化所有信号
- import 分层：数据类型从 `crate::models::*`，I/O 函数从 `crate::services::*`
- 异步方法签名为 `async fn xxx(&mut self, ...)`
- **不含 rsx!**，不含 DOM 操作
- spawn 内只使用 clone 的实例，不用 `use_xxx()` / `use_context()`
- **跨组件异步协作**：通过 `pending_send` 等信号传递请求，由 Layout（永不 unmount）消费并 spawn

### 3.4 Hooks

**规则**:

- `use_xxx()` — 获取 store 实例（页面/组件中使用）
- `use_xxx_provider()` — 初始化 store + 注入 Context + 加载初始数据（App 顶层调用）
- 不持有业务逻辑，只做桥接

### 3.5 Components

**规则**:

- 通过 `use_app()` / `use_conversation()` / `use_ui()` / `use_search()` 获取 store
- 不直接 `new` 任何 store
- 不直接调用 service 方法
- 图标通过 `crate::icons::*` 使用
- 样式通过 `#[with_css(css, "path/to/style.scss")]` + `css::class_name` 引用
- **JS 选择器不使用 CSS 类名**（dioxus_style 会哈希类名），改用 `data-*` 属性

### 3.6 Views

**规则**:

- 页面级组合组件，消费 stores
- 页面专属子组件放在对应视图子目录下（如 `views/conversation/chat_input.rs`）
- 不直接调用 service 方法
- Layout 负责处理跨组件异步协作（如 `pending_send`）

## 4. 文件命名

| 类型 | 规则 | 示例 |
|------|------|------|
| Model | 单数名词 | `ai.rs`, `config.rs`, `memory.rs` |
| Service | `{domain}.rs` 或 `{domain}/mod.rs` | `config.rs`, `ai/mod.rs`, `ollama/mod.rs` |
| Store | `{domain}.rs` | `app.rs`, `conversation.rs`, `search.rs` |
| Hook | `use_{domain}.rs` | `use_app.rs`, `use_search.rs` |
| Component | `snake_case.rs` 或 `{name}/mod.rs` | `sidebar.rs`, `input/mod.rs` |
| View | `{name}.rs` 或 `{name}/mod.rs` | `layout.rs`, `conversation/mod.rs` |
| Utility | 描述功能 | `html.rs`, `datetime.rs` |
| Style | 对应组件/视图名 | `conversation.scss`, `layout.scss` |

## 5. Import 规范

### 组件内 Import 顺序

```rust
use std::cell::RefCell;                                    // 标准库
use std::rc::Rc;
use dioxus::prelude::*;                                    // 框架
use dioxus_style::with_css;                                // 样式宏
use rust_i18n::t;                                          // i18n（如需要）
use crate::hooks::{use_app, use_conversation, use_ui};     // hooks
use crate::state::MainRoute;                               # 全局状态类型
use crate::stores::ui::{Toast, ToastKind};                 // stores 类型
use crate::icons::{Icon, tabler};                          // 图标
use crate::components::markdown::Markdown;                 // 子组件
```

### Stores 内 Import（分层引用）

```rust
use crate::models::ai::{ChatMessage, StreamEvent, SendMessageParams};
use crate::services::ai::{send_message, compress_messages};
```

## 6. 异步模式

### 6.1 组件内 spawn

```rust
// 闭包外 clone store
let mut conv_store = conv_store.clone();
// 闭包内 move + spawn
move |_| {
    spawn(async move {
        conv_store.send_message(...).await;
    });
}
```

### 6.2 跨组件异步协作（关键模式）

**问题**：Dioxus 的 `spawn` 绑定组件 scope，组件 unmount 时异步任务被取消。
Welcome 页 ChatInput 发送消息后 `navigate_to` 导致 unmount，`spawn` 的任务被取消。
`tokio::spawn` 不可用（`ConversationStore` 包含 `Signal`，非 `Send`）。

**解决方案**：通过 `pending_send` 信号传递请求，由 Layout 消费：

```rust
// ChatInput（可能被 unmount 的组件）
conv_store.pending_send.set(Some((content, config)));

// Layout（永不 unmount）
let pending = conv_store.pending_send.read().clone();
if let Some((content, config)) = pending {
    conv_store.pending_send.set(None);
    let mut conv_store = conv_store.clone();
    let toast_callback = move |kind, msg| { ... };
    spawn(async move {
        conv_store.send_message(content, config, toast_callback).await;
    });
}
```

### 6.3 禁止事项

- **禁止**在 `spawn` 内调用 `use_context()` / `use_xxx()`
- **禁止**使用 `tokio::spawn` 处理包含 `Signal` 的类型（非 `Send`）
- **禁止**在可能被 unmount 的组件中 spawn 长生命周期异步任务

## 7. 样式规范

- 使用 `#[with_css(css, "styles/xxx.scss")]` 宏注入作用域样式
- 类名通过 `css::class_name` 引用（dioxus_style 编译时哈希，不会与全局冲突）
- **JS 选择器禁止使用 CSS 类名**（dioxus_style 会哈希类名），改用 `data-*` 属性
- 内联颜色使用 CSS 变量 `var(--bg-dark)` 或 Rust 常量
- 禁止过长的内联 style 字符串
- 全局样式（theme、reset、keyframes、materials、utilities、markdown）在 `build.rs` 中编译为 `global.css`
- 组件/视图样式通过 dioxus_style `#[with_css]` 按需注入

### SCSS 文件组织

| 类型 | 路径 | 加载方式 |
|------|------|---------|
| 全局基础 | `styles/tokens.scss`, `theme.scss`, `reset.scss` 等 | `build.rs` 编译为 `global.css` |
| 组件样式 | `styles/components/*.scss` | dioxus_style `#[with_css]` 按需注入 |
| 视图样式 | `styles/views/*.scss` | dioxus_style `#[with_css]` 按需注入 |

## 8. i18n 规范

- 翻译文件：`locales/zh-CN.yml`、`locales/en.yml`
- 使用 `rust_i18n::t!` 宏：`t!("settings.title")`
- 键名层级：`{模块}.{功能}`，如 `chat.input-placeholder`、`settings.openai-compatible`
- 新增 UI 文本必须同步更新两个语言文件

## 9. 路由与导航

- 路由定义：`state.rs` 中的 `MainRoute` 枚举
- 导航方法：`app_store.navigate_to(MainRoute::*)`
- 路由匹配：`layout.rs` 中 `match route` 切换视图
- 新建对话：`conv_store.current_conversation_id.set(None)` + `navigate_to(MainRoute::Welcome)`
- 发送消息创建对话：`create_temporary_conversation()` + `navigate_to(MainRoute::Conversation(id))`

## 10. 文档注解规范

遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) 的文档约定。

### 必须注释的清单

- 所有 `pub struct` — 描述用途 + 每个字段用 `///` 行内文档
- 所有 `pub enum` — 描述每个变体含义
- 所有 `pub trait` — 描述契约 + 方法签名
- 所有 `pub fn` — 描述行为、参数语义、返回值、可能的错误
- 所有 `pub const` / `pub static` — 描述常量用途和默认值
- 所有 `impl` 块中的关键方法
- 所有 `#[component]` 函数 — 描述组件用途 + Props 字段文档
- 每个 `.rs` 文件顶部 — `//!` 模块注释

### 文档注释结构

```rust
/// 简短的一句话摘要（描述"做什么"，不超过一行）。
///
/// 详细描述（可选）。
///
/// # Arguments
///
/// * `param` - 参数说明
///
/// # Errors
///
/// 错误情况列举
```

### 禁止事项

- ❌ 公共 API 缺少 `///` 文档注释
- ❌ 用 `//` 替代 `///` 为 pub 类型/函数写说明
- ❌ 组件 Props 缺少 `///` 字段文档
- ❌ 文件缺少 `//!` 模块级注释
- ❌ 文档注释中包含过时信息

## 11. 禁止事项

1. **禁止** models/services 引用 `dioxus` UI 部分
2. **禁止** models 层包含 async I/O 逻辑
3. **禁止** 创建独立顶层模块（如 `src/ai/`）：必须归属到 `models/` 或 `services/`
4. **禁止** components 直接 `new` Store 或调用 service
5. **禁止** stores 中包含 `rsx!` 或 DOM 操作
6. **禁止** 在 `spawn` 内调用 `use_context()` / `use_xxx()`
7. **禁止** 使用 `tokio::spawn` 处理包含 `Signal` 的类型
8. **禁止** 文件名带冗余后缀（`_page`, `_store`）
9. **禁止** 创建 `README.md` 或文档文件（除非用户明确要求）
10. **禁止** 公共 API（pub struct/fn/trait/const）缺少 `///` 文档注释
11. **禁止** 使用过长的内联 style 字符串
12. **禁止** JS 选择器使用 dioxus_style 管理的 CSS 类名（改用 `data-*` 属性）
13. **禁止** 在可能被 unmount 的组件中 spawn 长生命周期异步任务

## 12. 修改检查清单

**每次修改 `.rs` 文件后，必须逐项检查：**

### 通用检查（所有场景）

1. [ ] 修改的文件是否在正确的层？（参考第 1 节五层架构）
2. [ ] 修改是否违反了依赖方向？（上层引用下层 ✓，下层引用上层 ✗）
3. [ ] 新增/修改的 `pub` 项是否有 `///` 文档注释？
4. [ ] 新增/修改的 `pub struct` 每个字段是否有 `///` 文档？
5. [ ] 修改的文件是否有 `//!` 模块级注释？
6. [ ] 新增的 `#[component]` 函数是否有文档注释 + Props 字段文档？
7. [ ] import 是否符合第 5 节的顺序规范？
8. [ ] 是否违反了第 11 节的任何禁止事项？
9. [ ] `cargo check` 是否零错误？
10. [ ] 异步任务是否在安全的 scope 中 spawn？（参考第 6 节）

### 新增功能额外检查

11. [ ] 数据结构在 `models/` 定义（纯 DTO + trait 签名）
12. [ ] I/O 逻辑在 `services/` 实现
13. [ ] Store 在 `stores/` 创建（持有 Signal，分层 import）
14. [ ] Hook 在 `hooks/` 创建 `use_xxx()` + `use_xxx_provider()`
15. [ ] 通用组件在 `components/` 创建，页面专属组件在 `views/{page}/` 下
16. [ ] 在 `lib.rs` / `mod.rs` 中声明模块 + 重导出
17. [ ] i18n 键是否同步更新了 `zh-CN.yml` 和 `en.yml`？
18. [ ] `cargo doc` 无警告

### BUG 修复额外检查

19. [ ] 修复是否引入了新的 `pub` 项？如果有，是否补全了文档注释？
20. [ ] 修复是否涉及跨层调用？是否违反了分层架构？
21. [ ] 修复是否使用了 JS 选择器？是否改用了 `data-*` 属性？
22. [ ] 修复是否添加了行内注释解释修复原因（why，而非 what）？
23. [ ] 修复是否涉及异步任务？spawn 位置是否安全？

### 重构额外检查

24. [ ] 重命名是否符合第 4 节文件命名规范？
25. [ ] 移动文件后是否更新了 `mod.rs` 壗明和 `pub use` 重导出？
26. [ ] 所有引用该模块的文件是否更新了 import 路径？

## 红线——停下来检查

如果你发现自己在想：

- "只是修个小 BUG，不需要检查文档注释" → **停下来，检查第 12 节清单**
- "这个改动太小，规范不适用" → **所有改动都适用，无例外**
- "文档注释可以后面再补" → **后面补 = 永远不会补，现在就做**
- "重构不需要更新文档" → **代码变了文档必须同步**
- "这个规范太严格了" → **严格是特性不是缺陷**
- "spawn 放这里应该没问题" → **检查组件是否可能被 unmount，参考第 6.2 节**

**以上所有都意味着：停下来，回到第 12 节检查清单。**

## 合理化借口表

| 借口 | 现实 |
|------|------|
| "只是修个 BUG" | BUG 修复也产生 pub 项，也需要文档注释 |
| "改动太小" | 一行修改也可能违反分层架构 |
| "后面再补文档" | 后面补 = 永远不会补，现在就做 |
| "规范太严格" | 严格保证一致性，一致性减少 BUG |
| "这个场景不同" | 没有特殊场景，所有 .rs 修改都适用 |
| "用户没要求" | 开发规范是自动应用的，不需要用户要求 |
| "我忘了检查" | 这正是检查清单存在的理由 |
| "spawn 应该没问题" | 组件 unmount 时 spawn 会被取消，参考第 6.2 节 |
| "tokio::spawn 更安全" | Signal 不是 Send，tokio::spawn 编译不过 |
