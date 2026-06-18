//! 聊天业务状态管理 Store。
//!
//! 持有对话列表、当前对话 ID、流式输出内容等聊天核心状态，
//! 提供消息发送、对话创建/重命名/删除、对话切换等业务方法。
//! 本模块属于 stores 层，依赖 `crate::models::ai` 提供的数据类型，
//! 以及 `crate::services::ai` / `crate::services::conversation_store` 提供的 I/O 函数。

use std::time::Duration;
use dioxus::prelude::*;
use crate::{Conversation, Message, MessageRole, MessageStatus, XEChatConfig};
use crate::models::ai::{ChatMessage, StreamEvent, SendMessageParams, DEFAULT_MAX_CONTEXT_TOKENS, DEFAULT_AUTO_CONTEXT_MANAGEMENT};
use crate::models::error::AppError;
use crate::services::ai::{send_message, compress_messages};
use crate::services::embedder::Embedder;
use crate::stores::ui::ToastKind;
use reqwest::Client;
use tokio_util::sync::CancellationToken;

pub use crate::services::conversation_store::SIDEBAR_MAX_CONVERSATIONS;

/// 重嵌入任务的事件，通过 channel 从 tokio 任务传递到 Dioxus spawn。
enum ReembedEvent {
    /// 进度更新 (current, total)
    Progress(usize, usize),
    /// 任务完成
    Done,
}

const FIRST_MESSAGE_SYSTEM_PROMPT: &str = concat!(
    "你是一位智能助手。请遵循以下规则：\n",
    "1. 用户的第一条消息是需要你回答的问题\n",
    "2. 在回复开头，先以 [TITLE:简短标题] 的格式输出一个对话标题（不超过15个字）\n",
    "3. 标题后换行，然后给出对用户的实际回复\n",
    "4. 标题应准确概括用户问题的核心主题"
);

/// 在对话列表中更新或插入对话。
///
/// 若 `conv_id` 已存在则替换，否则追加到末尾。
#[inline]
pub fn upsert_conversation(convs: &mut Vec<Conversation>, conv_id: &str, conv: Conversation) {
    if let Some(idx) = convs.iter().position(|c| c.id == conv_id) {
        convs[idx] = conv;
    } else {
        convs.push(conv);
    }
}

/// 从对话历史中提取 ChatMessage 列表。
///
/// 过滤掉空内容的助手消息，将 `MessageRole` 映射为字符串角色标识。
#[inline]
pub fn extract_history_messages(conv: &Conversation) -> Vec<ChatMessage> {
    conv.messages.iter()
        .filter(|m| m.role == MessageRole::User || (!m.content.is_empty() && m.role == MessageRole::Assistant))
        .map(|m| ChatMessage {
            role: if m.role == MessageRole::User { "user".into() } else { "assistant".into() },
            content: m.content.clone(),
        }).collect()
}

/// 获取记忆增强的前置消息列表。
///
/// 若记忆管线可用且使用了记忆，返回增强后的消息；否则返回空列表。
async fn get_memory_prepend(content: &str, recent_msgs: &[Message]) -> Vec<ChatMessage> {
    if let Some(pipeline) = crate::services::memory::get_pipeline() {
        let preprocess_result = pipeline.preprocess(content, recent_msgs).await;
        if preprocess_result.memory_used {
            return preprocess_result.enhanced_messages;
        }
    }
    Vec::new()
}

/// 首条消息持久化：创建新对话并保存到存储。
///
/// 成功返回 `true`，持久化失败返回 `false`。
async fn save_first_conversation(conv_id: &str, user_msg: &Message) -> bool {
    let new_conv = Conversation {
        id: conv_id.to_string(),
        title: String::from("New Chat"),
        messages: vec![user_msg.clone()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_temporary: false,
    };
    let save_result = if let Some(store) = crate::services::conversation_store::get_store() {
        store.save_conversation(&new_conv).await.map_err(|e| e.to_string())
    } else {
        Err("ConversationStore not initialized".to_string())
    };
    if let Err(e) = save_result {
        eprintln!("[xechat] Failed to create conversation: {}", e);
        return false;
    }
    true
}

pub fn parse_first_response(content: &str) -> (Option<String>, String) {
    let prefix = "[TITLE:";
    if let Some(start) = content.find(prefix)
        && start == 0
            && let Some(end) = content.find(']') {
                let title = content[prefix.len()..end].trim().to_string();
                let body = content[end + 1..].trim_start().to_string();
                return (Some(title), body);
            }
    (None, content.to_string())
}

/// 聊天业务状态 Store，管理对话和消息的完整生命周期。
///
/// 持有五个核心响应式信号：对话列表、当前对话 ID、流式输出内容、
/// 流式传输状态、用户滚动状态。通过 `send_message` 方法实现流式消息收发。
///
/// # 线程安全
///
/// 本类型实现 `Clone`，克隆后共享同一个底层 `Signal` 数据，
/// 确保多个组件实例看到的是同一个响应式状态。
/// 对话消息分页状态。
///
/// 采用窗口模式：`all_messages` 存储从 LanceDB 加载的完整消息列表（按 timestamp 升序），
/// `start_index` / `end_index` 控制当前可见的消息窗口范围。
/// 滚动到上/下边界时扩展窗口。
#[derive(Clone, Default)]
pub struct MessagePagination {
    /// 完整消息列表（按 timestamp 升序），从 LanceDB 一次性加载
    pub all_messages: Vec<Message>,
    /// 当前可见窗口的起始索引（含）
    pub start_index: usize,
    /// 当前可见窗口的结束索引（不含）
    pub end_index: usize,
    /// 每页大小
    pub page_size: usize,
    /// 是否正在加载中
    pub is_loading: bool,
}

#[derive(Clone)]
pub struct ConversationStore {
    /// 所有对话列表，按更新时间倒序排列
    pub conversations: Signal<Vec<Conversation>>,
    /// 当前激活的对话 ID（`None` 表示无选中对话）
    pub current_conversation_id: Signal<Option<String>>,
    /// 当前流式输出的文本片段（实时追加）
    pub streaming_content: Signal<String>,
    /// 当前流式输出的推理过程片段（实时追加）
    pub streaming_reasoning: Signal<String>,
    /// 是否正在进行 AI 流式回复
    pub is_streaming: Signal<bool>,
    /// 用户是否正在手动滚动消息区域
    pub is_user_scrolling: Signal<bool>,
    /// 共享 HTTP 客户端（连接池复用 + 请求超时配置）
    pub client: Client,
    /// 用于取消正在进行的流式请求
    cancel_token: Signal<Option<CancellationToken>>,
    /// 流式请求的 spawn 任务句柄，用于 abort HTTP 连接
    stream_task: Signal<Option<tokio::task::JoinHandle<()>>>,
    /// 当前对话的消息分页状态
    pub message_pagination: Signal<MessagePagination>,
    /// 待发送的消息（由 ChatInput 设置，由 Layout 消费并 spawn）
    /// 解决 Welcome 页 ChatInput 发送后 navigate 导致 spawn 被取消的问题
    pub pending_send: Signal<Option<(String, XEChatConfig)>>,
    /// 嵌入器是否就绪（模型已加载）
    pub embedder_ready: Signal<bool>,
    /// 轮次向量表是否因维度变更而重建（UI 据此显示 toast 提醒）
    pub turns_rebuilt: Signal<bool>,
    /// 是否正在重建轮次向量数据（切换 embedder 时）
    pub rebuild_in_progress: Signal<bool>,
    /// 重建进度 (current, total)
    pub rebuild_progress: Signal<(usize, usize)>,
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── 独立辅助函数（不依赖 self，可独立测试） ──────────────────────

/// 计算全量加载场景下的消息窗口范围。
///
/// 返回 `(start, end)` 索引，显示最新 `page_size` 条消息。
#[inline]
pub fn compute_full_window_range(total: usize, page_size: usize) -> (usize, usize) {
    let start = total.saturating_sub(page_size);
    (start, total)
}

/// 计算锚定加载场景下的消息窗口范围。
///
/// 返回 `(start, end)` 索引，从锚点消息开始显示 `page_size` 条。
#[inline]
pub fn compute_anchored_window_range(total: usize, page_size: usize, anchor_index: usize) -> (usize, usize) {
    let start = anchor_index;
    let end = std::cmp::min(start + page_size, total);
    (start, end)
}

/// 检查是否可以加载更早的消息，返回当前分页快照。
///
/// 若正在加载或已到顶部，返回 `None`。
#[inline]
pub fn can_load_older(pg: &MessagePagination) -> Option<(usize, usize, usize)> {
    if pg.is_loading || pg.start_index == 0 {
        return None;
    }
    Some((pg.start_index, pg.page_size, pg.all_messages.len()))
}

/// 计算加载更早消息后的新窗口起始索引。
#[inline]
pub fn compute_older_window(start: usize, page_size: usize) -> usize {
    start.saturating_sub(page_size)
}

/// 检查是否可以加载更晚的消息，返回当前分页快照。
///
/// 若正在加载或已到底部，返回 `None`。
#[inline]
pub fn can_load_newer(pg: &MessagePagination) -> Option<(usize, usize, usize)> {
    if pg.is_loading || pg.end_index >= pg.all_messages.len() {
        return None;
    }
    Some((pg.end_index, pg.page_size, pg.all_messages.len()))
}

/// 计算加载更晚消息后的新窗口结束索引。
#[inline]
pub fn compute_newer_window_end(end: usize, page_size: usize, all_len: usize) -> usize {
    std::cmp::min(end + page_size, all_len)
}

/// 同步 Ollama 主机配置到 provider（当使用 ollama 提供商时）。
#[inline]
pub fn sync_ollama_host_to_provider(provider: &mut crate::models::config::ModelProvider, config: &XEChatConfig) {
    if config.model_provider == "ollama" && !config.preferences.ollama.host.is_empty() {
        provider.base_url = config.preferences.ollama.host.clone();
    }
}

/// 判断 Ollama 嵌入器是否应启用（配置指定了 ollama 提供商且嵌入模型非空）。
#[inline]
pub fn should_enable_ollama(config: &crate::models::config::XEChatConfig) -> bool {
    config.preferences.embed_provider == "ollama" && !config.preferences.ollama.embed_model.is_empty()
}

/// 用户选择了 ollama 作为嵌入提供商（无论是否已配置具体模型）。
///
/// 用于区分"用户明确选择 ollama"与"使用默认内置模式"，防止在 ollama 模式下
/// 因 embed_model 尚未配置而静默 fallback 到内置 Qwen3 模型导致向量重建。
#[inline]
pub fn is_ollama_provider_selected(config: &crate::models::config::XEChatConfig) -> bool {
    config.preferences.embed_provider == "ollama"
}

// ── ConversationStore 实现 ────────────────────────────────────────

impl ConversationStore {
    /// 创建 ChatStore 实例并初始化所有信号为默认值。
    ///
    /// 默认值：
    /// - `conversations`: 空列表
    /// - `current_conversation_id`: `None`
    /// - `streaming_content`: 空字符串
    /// - `is_streaming`: `false`
    /// - `is_user_scrolling`: `false`
    pub fn new() -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(30))
            // 禁用连接池空闲超时：流式响应可能持续数分钟，
            // 默认 90s 空闲超时会导致长连接被提前关闭。
            .pool_idle_timeout(Duration::from_secs(0))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            conversations: Signal::new(Vec::new()),
            current_conversation_id: Signal::new(None),
            streaming_content: Signal::new(String::new()),
            streaming_reasoning: Signal::new(String::new()),
            is_streaming: Signal::new(false),
            is_user_scrolling: Signal::new(false),
            client,
            cancel_token: Signal::new(None),
            stream_task: Signal::new(None),
            message_pagination: Signal::new(MessagePagination {
                all_messages: Vec::new(),
                start_index: 0,
                end_index: 0,
                page_size: crate::services::conversation_store::DEFAULT_PAGE_SIZE,
                is_loading: false,
            }),
            pending_send: Signal::new(None),
            embedder_ready: Signal::new(false),
            turns_rebuilt: Signal::new(false),
            rebuild_in_progress: Signal::new(false),
            rebuild_progress: Signal::new((0, 0)),
        }
    }

    /// 获取当前选中的对话副本。
    ///
    /// # Returns
    ///
    /// 当前选中对话的克隆实例；若未选中任何对话则返回 `None`
    pub fn selected_conversation(&self) -> Option<Conversation> {
        let id = self.current_conversation_id.read();
        let id = id.as_ref()?;
        self.conversations.read().iter().find(|c| c.id == *id).cloned()
    }

    /// 检查是否正在进行流式传输。
    pub fn streaming(&self) -> bool {
        *self.is_streaming.read()
    }

    /// 停止当前流式接收，保留已生成内容并标记为截断状态。
    ///
    /// 调用 `CancellationToken::cancel()` 通知流式循环退出。
    /// 实际的保存和清理由 `send_message` 中的取消检测分支完成。
    pub fn stop_streaming(&mut self) {
        if let Some(token) = self.cancel_token.read().as_ref() {
            token.cancel();
        }
    }

    /// 获取流式内容的快照副本。
    pub fn stream_content_snapshot(&self) -> String {
        self.streaming_content.read().clone()
    }

    /// 从持久化存储加载对话列表到内存（sidebar 初始化用）。
    ///
    /// 使用 `load_sidebar_list` 仅加载 id/title/timestamp，不含消息和摘要。
    /// sidebar 组件通过 `SIDEBAR_MAX_CONVERSATIONS` 截取显示数量。
    pub async fn load_conversations(&mut self) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            if let Ok(convs) = store.load_sidebar_list().await {
                self.conversations.set(convs);
            }
        }
    }

    /// 计算全量加载场景下的消息窗口范围。
    ///
    /// 返回 `(start, end)` 索引，显示最新 `page_size` 条消息。
    #[inline]
    pub fn compute_full_window_range(total: usize, page_size: usize) -> (usize, usize) {
        compute_full_window_range(total, page_size)
    }

    /// 处理加载对话结果的公共逻辑：成功时计算窗口范围并应用。
    fn handle_load_result(
        &mut self,
        conv_id: &str,
        result: anyhow::Result<Option<Conversation>>,
        compute_range: impl Fn(usize, usize) -> (usize, usize),
        error_context: &str,
    ) {
        match result {
            Ok(Some(loaded_conv)) => {
                let total = loaded_conv.messages.len();
                let page_size = self.message_pagination.read().page_size;
                let (start, end) = compute_range(total, page_size);
                self.apply_windowed_conversation(conv_id, loaded_conv, start, end);
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("[xechat] {}: failed to load conv={}: {}", error_context, conv_id, e);
            }
        }
    }

    /// 加载指定对话的完整内容（含消息），滚动条置底。
    ///
    /// 全记录点击场景：加载所有消息，显示最新 size 条。
    pub async fn load_conversation_content(&mut self, conv_id: &str) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            let result = store.load_conversation_by_id(conv_id, crate::services::conversation_store::DEFAULT_PAGE_SIZE * 100).await;
            self.handle_load_result(conv_id, result, compute_full_window_range, "load_conversation_content");
        }
    }

    /// 计算锚定加载场景下的消息窗口范围。
    ///
    /// 返回 `(start, end)` 索引，从锚点消息开始显示 `page_size` 条。
    #[inline]
    pub fn compute_anchored_window_range(total: usize, page_size: usize, anchor_index: usize) -> (usize, usize) {
        compute_anchored_window_range(total, page_size, anchor_index)
    }

    /// 加载指定对话并定位到特定消息，定位消息置顶。
    ///
    /// 搜索匹配场景：加载定位消息及后续 size-1 条（共 size 条）。
    pub async fn load_conversation_content_anchored(&mut self, conv_id: &str, anchor_msg_id: &str) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            let result = store.load_conversation_by_id(conv_id, crate::services::conversation_store::DEFAULT_PAGE_SIZE * 100).await;
            if let Ok(Some(ref loaded_conv)) = result {
                let anchor_index = loaded_conv.messages.iter().position(|m| m.id == anchor_msg_id).unwrap_or(0);
                self.handle_load_result(conv_id, result, |total, page_size| {
                    compute_anchored_window_range(total, page_size, anchor_index)
                }, "load_conversation_content_anchored");
            } else {
                self.handle_load_result(conv_id, result, compute_full_window_range, "load_conversation_content_anchored");
            }
        }
    }

    /// 将加载的对话按窗口范围应用到分页状态和对话列表。
    ///
    /// 更新 `message_pagination` 的 `all_messages`、`start_index`、`end_index`，
    /// 并将窗口内的消息同步到 `conversations` 列表中对应的对话。
    fn apply_windowed_conversation(&mut self, conv_id: &str, loaded_conv: Conversation, start: usize, end: usize) {
        self.message_pagination.write().all_messages = loaded_conv.messages.clone();
        self.message_pagination.write().start_index = start;
        self.message_pagination.write().end_index = end;

        let windowed_conv = Conversation {
            id: loaded_conv.id.clone(),
            title: loaded_conv.title.clone(),
            messages: loaded_conv.messages[start..end].to_vec(),
            created_at: loaded_conv.created_at,
            updated_at: loaded_conv.updated_at,
            is_temporary: loaded_conv.is_temporary,
        };

        let mut convs = self.conversations.write();
        upsert_conversation(&mut convs, conv_id, windowed_conv);
    }

    /// 向上扩展消息窗口（加载更早的消息）。
    ///
    /// 滚动到顶部时触发，扩展 start_index。
    pub async fn load_more_messages_older(&mut self, conv_id: &str) {
        let (start, page_size, all_len) = match can_load_older(&self.message_pagination.read()) {
            Some(v) => v,
            None => return,
        };
        if all_len == 0 {
            return;
        }

        self.message_pagination.write().is_loading = true;

        let new_start = compute_older_window(start, page_size);
        let end = self.message_pagination.read().end_index;

        // 从 all_messages 中取出窗口
        let all_messages = self.message_pagination.read().all_messages.clone();
        let windowed_messages = all_messages[new_start..end].to_vec();

        self.message_pagination.write().start_index = new_start;
        self.message_pagination.write().is_loading = false;

        let mut convs = self.conversations.write();
        if let Some(idx) = convs.iter().position(|c| c.id == conv_id) {
            convs[idx].messages = windowed_messages;
        }
    }

    /// 向下扩展消息窗口（加载更晚的消息）。
    ///
    /// 滚动到底部时触发，扩展 end_index。
    pub async fn load_more_messages_newer(&mut self, conv_id: &str) {
        let (end, page_size, all_len) = match can_load_newer(&self.message_pagination.read()) {
            Some(v) => v,
            None => return,
        };
        if all_len == 0 {
            return;
        }

        self.message_pagination.write().is_loading = true;

        let new_end = compute_newer_window_end(end, page_size, all_len);
        let start = self.message_pagination.read().start_index;

        let all_messages = self.message_pagination.read().all_messages.clone();
        let windowed_messages = all_messages[start..new_end].to_vec();

        self.message_pagination.write().end_index = new_end;
        self.message_pagination.write().is_loading = false;

        let mut convs = self.conversations.write();
        if let Some(idx) = convs.iter().position(|c| c.id == conv_id) {
            convs[idx].messages = windowed_messages;
        }
    }

    /// 重置指定对话的分页状态。
    pub fn reset_pagination(&mut self) {
        self.message_pagination.set(MessagePagination {
            all_messages: Vec::new(),
            start_index: 0,
            end_index: 0,
            page_size: crate::services::conversation_store::DEFAULT_PAGE_SIZE,
            is_loading: false,
        });
    }

    /// 创建新对话并插入到对话列表头部。
    ///
    /// # Arguments
    ///
    /// * `title` - 对话标题
    ///
    /// # Returns
    ///
    /// - `Ok(Conversation)` — 新创建的对话实例
    /// - `Err(AppError)` — 持久化失败时的结构化错误
    pub async fn create_conversation(&mut self, title: &str) -> Result<Conversation, AppError> {
        let store = crate::services::conversation_store::get_store()
            .ok_or_else(|| AppError::Io { operation: "create conversation".into(), detail: "ConversationStore not initialized".into() })?;
        let conv = store.create_conversation(title).await
            .map_err(|e| AppError::Io { operation: "create conversation".into(), detail: e.to_string() })?;
        let mut convs = self.conversations.write();
        convs.insert(0, conv.clone());
        Ok(conv)
    }

    /// 切换当前选中的对话并重置流式状态。
    ///
    /// 切换时会清理未发送消息的临时对话（内存-only，不保留）。
    ///
    /// # Arguments
    ///
    /// * `id` - 目标对话 ID
    pub fn select_conversation(&mut self, id: String) {
        if let Some(token) = self.cancel_token.read().as_ref() {
            token.cancel();
        }
        self.cancel_token.set(None);
        self.stream_task.set(None);

        let mut convs = self.conversations.write();
        convs.retain(|c| !c.is_temporary || c.id == id);
        drop(convs);

        self.current_conversation_id.set(Some(id));
        self.streaming_content.set(String::new());
        self.streaming_reasoning.set(String::new());
        self.is_streaming.set(false);
    }

    pub fn create_temporary_conversation(&mut self, title: String) -> String {
        let temp_conv = Conversation::new_temporary(title);
        let conv_id = temp_conv.id.clone();
        self.conversations.write().insert(0, temp_conv);
        self.current_conversation_id.set(Some(conv_id.clone()));
        conv_id
    }

    pub async fn init_backend(&mut self) {
        let config = crate::services::config::load_config().unwrap_or_default();

        // 阶段 1：初始化嵌入器
        let embedder_ready = self.init_embedder(&config).await;

        // 阶段 2：初始化 LanceDB + ConversationStore（不依赖 embedder）
        let conv_lancedb_path = crate::services::paths::get_lancedb_path();
        std::fs::create_dir_all(&conv_lancedb_path).ok();
        let conv_lancedb_str = conv_lancedb_path.to_str().unwrap_or("").to_string();

        // 初始化向量存储（turns 表）— 仅在 embedder 就绪时创建
        let (vector_store, raw_turns) = Self::init_vector_store(&conv_lancedb_str, embedder_ready).await;
        if raw_turns.is_some() {
            self.turns_rebuilt.set(true);
        }

        // 初始化记忆管线（需要 embedder + vector_store）
        Self::init_memory_pipeline(&vector_store);

        // 初始化 ConversationStore
        self.init_conversation_store(&conv_lancedb_str, vector_store).await;

        // 启动时如有待重嵌入数据，异步执行
        if let Some(turns) = raw_turns {
            eprintln!("[xechat] init_backend: {} raw turns to re-embed", turns.len());
            self.spawn_reembed_task(turns, false);
        }
    }

    /// 初始化 ConversationStore 并加载对话列表。
    ///
    /// 成功时初始化全局 store 并加载对话，失败时输出错误日志。
    async fn init_conversation_store(
        &mut self,
        lancedb_path: &str,
        vector_store: Option<std::sync::Arc<dyn crate::services::vector_store::VectorStore>>,
    ) {
        match Self::open_conversation_store(lancedb_path, vector_store).await {
            Ok(store) => {
                if let Err(e) = crate::services::conversation_store::init_store(store) {
                    eprintln!("[xechat] Failed to init conversation store: {}", e);
                }
                self.load_conversations().await;
            }
            Err(e) => {
                eprintln!("[xechat] Failed to open LanceDB for conversations: {}", e);
            }
        }
    }

    /// 重新初始化嵌入器和向量存储。
    ///
    /// 当用户在设置中切换嵌入提供商时调用。
    /// 重新初始化 embedder → 检测变更 → 重建 turns 表（如需要）→ 重嵌入数据 → 更新记忆管线。
    /// 返回 `true` 表示 turns 表已重建。
    pub async fn reinit_embedder(&mut self) -> bool {
        let config = crate::services::config::load_config().unwrap_or_default();

        // 重新初始化嵌入器
        let embedder_ready = self.init_embedder(&config).await;

        // 重新打开向量存储并检测变更
        let conv_lancedb_path = crate::services::paths::get_lancedb_path();
        let conv_lancedb_str = conv_lancedb_path.to_str().unwrap_or("").to_string();

        let (vector_store, raw_turns) = Self::init_vector_store(&conv_lancedb_str, embedder_ready).await;

        // 更新记忆管线
        Self::init_memory_pipeline(&vector_store);

        // 更新全局 ConversationStore 的 vector_store
        crate::services::conversation_store::update_vector_store(vector_store);

        let rebuilt = raw_turns.is_some();
        if rebuilt {
            self.turns_rebuilt.set(true);
        }

        // 如有待重嵌入数据，异步执行
        match raw_turns {
            Some(turns) => {
                eprintln!("[xechat] reinit_embedder: {} raw turns to re-embed", turns.len());
                self.spawn_reembed_task(turns, true);
            }
            None => {
                eprintln!("[xechat] reinit_embedder: no raw turns to re-embed");
            }
        }

        rebuilt
    }

    /// 打开 LanceDbStore 并确保表存在，用于重嵌入任务。
    pub async fn open_store_for_reembed(lancedb_path: &str) -> anyhow::Result<crate::services::vector_store::lancedb_store::LanceDbStore> {
        let mut vs = crate::services::vector_store::lancedb_store::LanceDbStore::open(lancedb_path).await?;
        vs.ensure_table().await?;
        match vs.count_turns_rows().await {
            Ok(n) => eprintln!("[xechat] spawn_reembed: after ensure_table, row_count={}", n),
            Err(e) => eprintln!("[xechat] spawn_reembed: failed to count rows: {}", e),
        }
        Ok(vs)
    }

    /// 从对话存储加载轮次数据，用于重嵌入任务。
    pub async fn load_turns_for_reembed() -> anyhow::Result<Vec<crate::services::vector_store::lancedb_store::RawTurn>> {
        let store = crate::services::conversation_store::get_store()
            .ok_or_else(|| anyhow::anyhow!("no conversation store available"))?;
        let turns = store.load_all_turns_from_conversations().await?;
        eprintln!("[xechat] Extracted {} turns from conversations", turns.len());
        Ok(turns)
    }

    /// 重嵌入完成后更新全局 vector_store 和 memory pipeline。
    pub fn update_global_store_after_reembed(vs: crate::services::vector_store::lancedb_store::LanceDbStore) {
        let new_vs: std::sync::Arc<dyn crate::services::vector_store::VectorStore> =
            std::sync::Arc::new(vs);
        crate::services::conversation_store::update_vector_store(Some(new_vs.clone()));
        if let Some(embedder) = crate::services::embedder::get_embedder() {
            let pipeline = crate::services::memory::MemoryPipeline::new(embedder, new_vs);
            if let Err(e) = crate::services::memory::init_pipeline(pipeline) {
                eprintln!("[xechat] Failed to update memory pipeline after re-embed: {}", e);
            }
        }
        eprintln!("[xechat] Global vector_store and memory pipeline updated after re-embed");
    }

    /// 处理重嵌入结果：日志输出 + 更新全局 store。
    fn handle_reembed_result(
        result: anyhow::Result<(usize, usize)>,
        vs: crate::services::vector_store::lancedb_store::LanceDbStore,
    ) {
        match result {
            Ok((success, skipped)) => {
                if skipped > 0 {
                    eprintln!("[xechat] Re-embed partial: {} succeeded, {} skipped", success, skipped);
                } else {
                    eprintln!("[xechat] Re-embed completed successfully ({} turns)", success);
                }
                Self::update_global_store_after_reembed(vs);
            }
            Err(e) => eprintln!("[xechat] Re-embed failed: {}", e),
        }
    }

    /// 执行重嵌入核心逻辑：获取 embedder → 打开 store → 加载 turns → reembed → 处理结果。
    ///
    /// 函数退出前统一发送 `ReembedEvent::Done`。
    async fn run_reembed_core(
        lancedb_path: String,
        force_rebuild: bool,
        tx: tokio::sync::mpsc::Sender<ReembedEvent>,
    ) {
        let (embedder, vs, raw_turns) = match Self::prepare_reembed_params(&lancedb_path, &tx).await {
            Some(params) => params,
            None => return,
        };

        let result = Self::execute_reembed_batch(&vs, raw_turns, &embedder, &tx, force_rebuild).await;

        let _ = tx.send(ReembedEvent::Done).await;
        Self::handle_reembed_result(result, vs);
    }

    /// 准备重嵌入参数：获取 embedder、打开 store、加载 turns。
    ///
    /// 任一步骤失败时发送 `ReembedEvent::Done` 并返回 `None`。
    /// 获取嵌入器，失败时发送 Done 事件。
    async fn get_embedder_or_fail(tx: &tokio::sync::mpsc::Sender<ReembedEvent>) -> Option<std::sync::Arc<dyn crate::services::embedder::Embedder>> {
        match crate::services::embedder::get_embedder() {
            Some(e) => Some(e),
            None => {
                eprintln!("[xechat] Re-embed failed: no embedder available");
                let _ = tx.send(ReembedEvent::Done).await;
                None
            }
        }
    }

    /// 打开向量存储，失败时发送 Done 事件。
    async fn open_store_or_fail(lancedb_path: &str, tx: &tokio::sync::mpsc::Sender<ReembedEvent>) -> Option<crate::services::vector_store::lancedb_store::LanceDbStore> {
        match Self::open_store_for_reembed(lancedb_path).await {
            Ok(vs) => {
                eprintln!("[xechat] spawn_reembed: LanceDbStore opened, dim={}", vs.vector_dim());
                Some(vs)
            }
            Err(e) => {
                eprintln!("[xechat] Re-embed failed: {}", e);
                let _ = tx.send(ReembedEvent::Done).await;
                None
            }
        }
    }

    /// 加载轮次数据，失败或为空时发送 Done 事件。
    async fn load_turns_or_fail(tx: &tokio::sync::mpsc::Sender<ReembedEvent>) -> Option<Vec<crate::services::vector_store::lancedb_store::RawTurn>> {
        match Self::load_turns_for_reembed().await {
            Ok(t) if !t.is_empty() => Some(t),
            Ok(_) => {
                eprintln!("[xechat] No turns to re-embed");
                let _ = tx.send(ReembedEvent::Done).await;
                None
            }
            Err(e) => {
                eprintln!("[xechat] Re-embed failed: {}", e);
                let _ = tx.send(ReembedEvent::Done).await;
                None
            }
        }
    }

    async fn prepare_reembed_params(
        lancedb_path: &str,
        tx: &tokio::sync::mpsc::Sender<ReembedEvent>,
    ) -> Option<(
        std::sync::Arc<dyn crate::services::embedder::Embedder>,
        crate::services::vector_store::lancedb_store::LanceDbStore,
        Vec<crate::services::vector_store::lancedb_store::RawTurn>,
    )> {
        let embedder = Self::get_embedder_or_fail(tx).await?;
        let vs = Self::open_store_or_fail(lancedb_path, tx).await?;
        let raw_turns = Self::load_turns_or_fail(tx).await?;
        Some((embedder, vs, raw_turns))
    }

    /// 执行批量重嵌入，通过 channel 报告进度。
    async fn execute_reembed_batch(
        vs: &crate::services::vector_store::lancedb_store::LanceDbStore,
        raw_turns: Vec<crate::services::vector_store::lancedb_store::RawTurn>,
        embedder: &std::sync::Arc<dyn crate::services::embedder::Embedder>,
        tx: &tokio::sync::mpsc::Sender<ReembedEvent>,
        force_rebuild: bool,
    ) -> anyhow::Result<(usize, usize)> {
        let tx_progress = tx.clone();
        vs.reembed_turns(raw_turns, embedder, &|current, total| {
            let _ = tx_progress.try_send(ReembedEvent::Progress(current, total));
        }, force_rebuild).await
    }

    /// 启动异步重嵌入任务。
    ///
    /// 显示重建进度遮罩，逐条用新 embedder 重新分块和嵌入，
    /// 完成后关闭遮罩并显示成功提示。
    fn spawn_reembed_task(&mut self, raw_turns: Vec<crate::services::vector_store::lancedb_store::RawTurn>, force_rebuild: bool) {
        let total = raw_turns.len();
        eprintln!("[xechat] spawn_reembed_task: {} raw turns from turns table", total);

        // 即使 turns 表为空，也启动任务（会从对话消息中提取轮次）
        self.rebuild_in_progress.set(true);
        self.rebuild_progress.set((0, total.max(1)));

        let lancedb_path = crate::services::paths::get_lancedb_path()
            .to_str().unwrap_or("").to_string();

        // 使用 channel 传递进度和完成信号，避免 Signal 跨线程
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ReembedEvent>(100);
        let mut rebuild_in_progress = self.rebuild_in_progress;
        let mut rebuild_progress = self.rebuild_progress;

        // 进度更新 + 完成信号：Dioxus spawn 中更新 Signal
        spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    ReembedEvent::Progress(current, total) => {
                        rebuild_progress.set((current, total));
                    }
                    ReembedEvent::Done => {
                        rebuild_in_progress.set(false);
                        break;
                    }
                }
            }
        });

        // 实际重嵌入：tokio::spawn 中执行
        tokio::spawn(Self::run_reembed_core(lancedb_path, force_rebuild, tx));
    }

    /// 初始化向量存储（turns 表），仅在 embedder 就绪时创建。
    ///
    /// 返回 `(vector_store, raw_turns)`：
    /// - `vector_store`：成功时为 `Some`，否则 `None`
    /// - `raw_turns`：因 embedder 变更而需要重嵌入的原始轮次数据（`Some(vec![])` 表示无数据需重嵌入）
    async fn init_vector_store(
        lancedb_path: &str,
        embedder_ready: bool,
    ) -> (Option<std::sync::Arc<dyn crate::services::vector_store::VectorStore>>, Option<Vec<crate::services::vector_store::lancedb_store::RawTurn>>) {
        if !embedder_ready {
            return (None, None);
        }
        match crate::services::vector_store::lancedb_store::LanceDbStore::open(lancedb_path).await {
            Ok(mut vs) => {
                Self::ensure_turns_table_async(&mut vs).await;
                let raw_turns = Self::check_and_rebuild_dimension_mismatch(&mut vs).await;
                (Some(std::sync::Arc::new(vs)), raw_turns)
            }
            Err(e) => {
                eprintln!("[xechat] Failed to open LanceDbStore for turns: {}", e);
                (None, None)
            }
        }
    }

    /// 检测 embedder 变更或维度不匹配，需要时重建 turns 表。
    ///
    /// 重建前先读取原始轮次文本数据，用于后续重嵌入。
    /// 返回 `Some(raw_turns)` 表示需要重嵌入（可能为空 vec，会从对话消息重建），
    /// `None` 表示无需重建。
    async fn check_and_rebuild_dimension_mismatch(
        vs: &mut crate::services::vector_store::lancedb_store::LanceDbStore,
    ) -> Option<Vec<crate::services::vector_store::lancedb_store::RawTurn>> {
        // 检查 embedder 是否变更（名称+维度）
        if let Some(current_meta) = vs.check_embedder_changed() {
            eprintln!(
                "[xechat] Embedder changed, rebuilding turns table: name={} dim={}",
                current_meta.name, current_meta.dimension
            );
            let raw_turns = Self::rebuild_turns_table(vs, current_meta.dimension, Some(current_meta)).await;
            return Some(raw_turns);
        }

        // 检查维度是否匹配（兼容旧版本无元数据文件的情况）
        let embedder_dim = match Self::detect_embedder_dim() {
            Some(d) => d,
            None => return None,
        };

        if vs.vector_dim() != embedder_dim {
            eprintln!(
                "[xechat] Dimension mismatch: turns table={} embedder={}, rebuilding turns table",
                vs.vector_dim(), embedder_dim
            );
            let meta = crate::services::embedder::get_embedder().map(|e| {
                crate::services::vector_store::lancedb_store::EmbedderMeta {
                    name: e.name().to_string(),
                    dimension: e.dimension() as i32,
                }
            });
            let raw_turns = Self::rebuild_turns_table(vs, embedder_dim, meta).await;
            return Some(raw_turns);
        }

        // 维度匹配，但 turns 表可能为空（旧版本删表后未重建）
        Self::check_empty_turns_table(vs).await
    }

    /// 检测当前 embedder 的向量维度。
    ///
    /// 返回 `Some(dim)` 表示 embedder 可用，`None` 表示无 embedder。
    fn detect_embedder_dim() -> Option<i32> {
        crate::services::embedder::get_embedder().map(|e| e.dimension() as i32)
    }

    /// 重建 turns 表：读取原始数据 → 删表 → 设置维度 → 写入元数据 → 重建表。
    ///
    /// 返回读取到的原始轮次数据。
    async fn rebuild_turns_table(
        vs: &mut crate::services::vector_store::lancedb_store::LanceDbStore,
        new_dim: i32,
        meta: Option<crate::services::vector_store::lancedb_store::EmbedderMeta>,
    ) -> Vec<crate::services::vector_store::lancedb_store::RawTurn> {
        // 先读取原始数据，再删表
        let raw_turns = vs.read_all_turns_raw().await.unwrap_or_default();
        eprintln!("[xechat] Read {} raw turns for re-embedding", raw_turns.len());
        vs.drop_and_recreate_turns_table().await;
        vs.set_vector_dim(new_dim);

        // 写入元数据（如有）
        if let Some(m) = meta {
            vs.write_embedder_meta(&m);
        }

        // 重建后需要重新初始化 turns_table，否则搜索时 turns_table 为 None
        let _ = vs.ensure_table().await;
        raw_turns
    }

    /// 检查 turns 表是否为空，空表需要从对话消息重建轮次向量。
    async fn check_empty_turns_table(
        vs: &crate::services::vector_store::lancedb_store::LanceDbStore,
    ) -> Option<Vec<crate::services::vector_store::lancedb_store::RawTurn>> {
        let row_count = vs.count_turns_rows().await.unwrap_or(0);
        if row_count > 0 {
            return None;
        }
        eprintln!("[xechat] Turns table is empty ({} rows), will rebuild from conversations", row_count);
        Some(Vec::new())
    }

    /// 确保 turns 表存在。
    async fn ensure_turns_table_async(vs: &mut crate::services::vector_store::lancedb_store::LanceDbStore) {
        if let Err(e) = vs.ensure_table().await {
            eprintln!("[xechat] Failed to ensure turns table: {}", e);
        }
    }

    /// 增量重建向量：保留已有数据，仅嵌入缺失的轮次。
    ///
    /// 与 `reinit_embedder()` 不同，本方法**不会**删除 turns 表，
    /// 因此 `reembed_turns` 中的断点续传逻辑可以正确跳过已存在的 turn。
    ///
    /// 适用场景：用户手动点击"重建向量"按钮重试上次失败的轮次。
    pub async fn rebuild_vectors(&mut self) {
        self.rebuild_in_progress.set(true);
        self.rebuild_progress.set((0, 1));

        // 从对话消息中提取所有轮次（数据源，始终使用）
        let raw_turns = match self.load_turns_for_rebuild().await {
            Some(turns) => turns,
            None => {
                self.rebuild_in_progress.set(false);
                return;
            }
        };

        if raw_turns.is_empty() {
            eprintln!("[xechat] rebuild_vectors: no turns to process");
            self.rebuild_in_progress.set(false);
            return;
        }

        // 复用 spawn_reembed_task 执行实际的强制重建
        self.spawn_reembed_task(raw_turns, true);
    }

    /// 从对话存储加载轮次数据，用于重建向量。
    ///
    /// 返回 `Some(turns)` 表示加载成功，`None` 表示存储不可用或加载失败。
    async fn load_turns_for_rebuild(&self) -> Option<Vec<crate::services::vector_store::lancedb_store::RawTurn>> {
        let store = match crate::services::conversation_store::get_store() {
            Some(s) => s,
            None => {
                eprintln!("[xechat] rebuild_vectors: no conversation store");
                return None;
            }
        };
        match store.load_all_turns_from_conversations().await {
            Ok(turns) => {
                eprintln!("[xechat] rebuild_vectors: extracted {} turns from conversations", turns.len());
                Some(turns)
            }
            Err(e) => {
                eprintln!("[xechat] rebuild_vectors: failed to load conversations: {}", e);
                None
            }
        }
    }

    /// 初始化记忆管线（需要 embedder + vector_store 同时就绪）。
    fn init_memory_pipeline(vector_store: &Option<std::sync::Arc<dyn crate::services::vector_store::VectorStore>>) {
        if let (Some(embedder), Some(vs)) = (crate::services::embedder::get_embedder(), vector_store.as_ref()) {
            let pipeline = crate::services::memory::MemoryPipeline::new(embedder, vs.clone());
            if let Err(e) = crate::services::memory::init_pipeline(pipeline) {
                eprintln!("[xechat] Failed to init memory pipeline: {}", e);
            }
        }
    }

    /// 打开并初始化 ConversationStore（含 ensure_table）。
    async fn open_conversation_store(
        lancedb_path: &str,
        vector_store: Option<std::sync::Arc<dyn crate::services::vector_store::VectorStore>>,
    ) -> anyhow::Result<crate::services::conversation_store::ConversationStore> {
        let mut store = crate::services::conversation_store::ConversationStore::open(lancedb_path, vector_store).await?;
        if let Err(e) = store.ensure_table().await {
            eprintln!("[xechat] Failed to ensure conversations table: {}", e);
        }
        Ok(store)
    }

    /// 初始化嵌入器。
    ///
    /// 返回 `true` 表示 embedder 就绪，`false` 表示加载失败。
    /// 失败时设置 `embedder_ready` 信号为 false，但不影响后续初始化。
    ///
    /// 策略：
    /// 1. embed_provider = "ollama" 且配置有效 → 直接使用 OllamaEmbedder
    /// 2. 内置 Qwen3-Embedding-0.6B 可用 → 使用 Qwen3Embedder
    /// 3. 两者都不可用 → 返回 false
    async fn init_embedder(&mut self, config: &crate::models::config::XEChatConfig) -> bool {
        // 优先级 1：用户明确选择 ollama 作为嵌入提供商
        if should_enable_ollama(config) {
            return self.try_init_ollama_or_fail(config).await;
        }

        // [加固] 用户选择了 ollama 但尚未配置具体模型 → 不 fallback 到内置模型
        // 场景：用户选了 ollama provider 但还没选 model 就关闭了应用
        if is_ollama_provider_selected(config) {
            eprintln!(
                "[xechat] Ollama selected as embed provider but no model configured yet, \
                 skipping init (will retry after user selects a model)"
            );
            self.embedder_ready.set(false);
            return false;
        }

        // 优先级 2：内置 Qwen3-Embedding-0.6B（仅在非 ollama 模式下）
        self.init_builtin_embedder().await
    }

    /// 尝试将 Ollama 作为主嵌入器初始化，失败时不 fallback 到内置模型。
    ///
    /// Ollama 不可用时不 fallback，避免 embedder 名称不匹配导致向量重建。
    /// 用户需先启动 Ollama 再使用。
    async fn try_init_ollama_or_fail(&mut self, config: &crate::models::config::XEChatConfig) -> bool {
        if self.try_init_ollama_primary(config).await {
            return true;
        }
        eprintln!("[xechat] Ollama embedder unavailable, not falling back to built-in model to prevent vector rebuild");
        self.embedder_ready.set(false);
        false
    }

    /// 初始化内置 Qwen3-Embedding-0.6B 嵌入器。
    ///
    /// 通过 `spawn_blocking` 在后台线程加载模型，避免阻塞异步运行时。
    async fn init_builtin_embedder(&mut self) -> bool {
        match tokio::task::spawn_blocking(crate::services::embedder::qwen3::Qwen3Embedder::new).await {
            Ok(Ok(qwen3)) => {
                eprintln!("[xechat] Qwen3-Embedding-0.6B ready (dim={})", qwen3.dimension());
                let embedder: std::sync::Arc<dyn crate::services::embedder::Embedder> =
                    std::sync::Arc::new(qwen3);
                self.register_embedder_or_fail(embedder)
            }
            Ok(Err(e)) => {
                eprintln!("[xechat] Failed to init Qwen3 embedder: {}", e);
                self.embedder_ready.set(false);
                false
            }
            Err(e) => {
                eprintln!("[xechat] Qwen3 embedder task panicked: {}", e);
                self.embedder_ready.set(false);
                false
            }
        }
    }

    /// 注册嵌入器到全局单例，失败时设置 `embedder_ready` 为 false。
    ///
    /// 返回 `true` 表示注册成功，`false` 表示失败。
    fn register_embedder_or_fail(&mut self, embedder: std::sync::Arc<dyn crate::services::embedder::Embedder>) -> bool {
        if let Err(e) = crate::services::embedder::init_embedder(embedder) {
            eprintln!("[xechat] Embedder init error: {}", e);
            self.embedder_ready.set(false);
            return false;
        }
        self.embedder_ready.set(true);
        true
    }

    /// 尝试将 Ollama 作为主嵌入器初始化。
    ///
    /// 当用户明确选择 ollama 作为嵌入提供商时调用。
    /// 成功时设置 `embedder_ready = true` 并返回 `true`。
    async fn try_init_ollama_primary(&mut self, config: &crate::models::config::XEChatConfig) -> bool {
        let ollama_host = Self::resolve_ollama_host(&config.preferences.ollama.host);
        let embed_model = &config.preferences.ollama.embed_model;

        match crate::services::ollama::embed::OllamaEmbedder::probe(ollama_host, embed_model).await {
            Ok(ollama) => {
                eprintln!(
                    "[xechat] Ollama embedder ready: {} (dim={})",
                    embed_model,
                    ollama.dimension()
                );
                let embedder: std::sync::Arc<dyn crate::services::embedder::Embedder> =
                    std::sync::Arc::new(ollama);

                if let Err(e) = crate::services::embedder::init_embedder(embedder) {
                    eprintln!("[xechat] Embedder init error: {}", e);
                    self.embedder_ready.set(false);
                    return false;
                }

                self.embedder_ready.set(true);
                true
            }
            Err(e) => {
                eprintln!(
                    "[xechat] Ollama probe failed ({}): {}. Ollama must be running before using ollama embed provider.",
                    embed_model, e
                );
                false
            }
        }
    }

    /// 解析 Ollama 主机地址，空字符串时使用默认地址。
    pub fn resolve_ollama_host<'a>(configured_host: &'a str) -> &'a str {
        if configured_host.is_empty() {
            "http://localhost:11434"
        } else {
            configured_host
        }
    }

    /// 判断 Ollama 嵌入器是否应启用（配置指定了 ollama 提供商且嵌入模型非空）。
    #[inline]
    pub fn should_enable_ollama(config: &crate::models::config::XEChatConfig) -> bool {
        should_enable_ollama(config)
    }

    /// 重命名指定对话并同步更新内存状态和时间戳。
    ///
    /// # Arguments
    ///
    /// * `id` - 对话 ID
    /// * `new_title` - 新标题
    ///
    /// # Errors
    ///
    /// 返回 `Err` 当持久化写入失败
    pub async fn rename_conversation(&mut self, id: &str, new_title: &str) -> Result<(), AppError> {
        let store = crate::services::conversation_store::get_store()
            .ok_or_else(|| AppError::Io { operation: "rename conversation".into(), detail: "ConversationStore not initialized".into() })?;
        store.rename_conversation(id, new_title).await
            .map_err(|e| AppError::Io { operation: "rename conversation".into(), detail: e.to_string() })?;
        let mut convs = self.conversations.write();
        if let Some(conv) = convs.iter_mut().find(|c| c.id == id) {
            conv.title = new_title.to_string();
            conv.updated_at = chrono::Utc::now();
        }
        Ok(())
    }

    /// 删除指定对话并清理关联状态。
    ///
    /// 若被删除的是当前选中对话，则自动清除 `current_conversation_id`。
    ///
    /// # Arguments
    ///
    /// * `id` - 对话 ID
    ///
    /// # Errors
    ///
    /// 返回 `Err` 当持久化删除失败
    pub async fn delete_conversation(&mut self, id: &str) -> Result<(), AppError> {
        let store = crate::services::conversation_store::get_store()
            .ok_or_else(|| AppError::Io { operation: "delete conversation".into(), detail: "ConversationStore not initialized".into() })?;
        store.delete_conversation(id).await
            .map_err(|e| AppError::Io { operation: "delete conversation".into(), detail: e.to_string() })?;
        let mut convs = self.conversations.write();
        convs.retain(|c| c.id != id);
        if self.current_conversation_id.read().as_ref() == Some(&id.to_string()) {
            self.current_conversation_id.set(None);
        }
        Ok(())
    }

    /// 将消息追加到内存中的对话并更新时间戳。
    fn push_message_to_conversation(&mut self, conv_id: &str, msg: Message) {
        let mut convs = self.conversations.write();
        if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
            conv.messages.push(msg);
            conv.updated_at = chrono::Utc::now();
        }
    }

    /// 按更新时间降序排列对话列表。
    fn sort_conversations_by_updated_at(&mut self) {
        let mut convs = self.conversations.write();
        convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    }

    /// 持久化首条用户消息：保存对话到存储并标记为非临时。
    fn mark_conversation_permanent(&mut self, conv_id: &str) {
        let mut convs = self.conversations.write();
        if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
            conv.is_temporary = false;
        }
    }

    /// 追加后续用户消息到持久化存储。
    async fn append_message_to_store(conv_id: &str, user_msg: &Message) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            let _ = store.add_message(conv_id, user_msg).await;
        }
    }

    /// 持久化用户消息：首条消息创建对话，后续消息追加。
    ///
    /// 返回 `false` 表示首条消息持久化失败，调用方应中止发送流程。
    async fn persist_user_message(&mut self, conv_id: &str, user_msg: &Message, is_first: bool) -> bool {
        if is_first {
            if !save_first_conversation(conv_id, user_msg).await {
                return false;
            }
            self.mark_conversation_permanent(conv_id);
        } else {
            Self::append_message_to_store(conv_id, user_msg).await;
        }
        true
    }

    /// 构建发送给 AI 的消息列表（含记忆增强、系统提示、上下文压缩）。
    async fn build_chat_messages(&self, content: &str, config: &XEChatConfig, is_first: bool) -> Vec<ChatMessage> {
        let recent_msgs: Vec<Message> = self.selected_conversation()
            .map(|c| c.messages.clone())
            .unwrap_or_default();

        let memory_prepend = get_memory_prepend(content, &recent_msgs).await;

        let mut msgs: Vec<ChatMessage> = Vec::new();

        if !memory_prepend.is_empty() {
            msgs.extend(memory_prepend);
        }

        if is_first {
            msgs.push(ChatMessage {
                role: "system".into(),
                content: FIRST_MESSAGE_SYSTEM_PROMPT.to_string(),
            });
        }

        if let Some(conv) = self.selected_conversation() {
            let history = extract_history_messages(&conv);
            msgs.extend(history);
        }

        let max_tokens = config.max_context_tokens.unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS);
        let auto_management = config.auto_context_management.unwrap_or(DEFAULT_AUTO_CONTEXT_MANAGEMENT);
        compress_messages(&msgs, max_tokens, auto_management)
    }

    /// 处理流式取消：保存已生成内容为截断消息。
    async fn handle_stream_cancel(&mut self, conv_id: &str, full_content: &str) {
        let saved_content = if !full_content.is_empty() {
            full_content.to_string()
        } else {
            self.streaming_content.read().clone()
        };
        if saved_content.is_empty() {
            return;
        }
        let mut truncated_msg = Message::new_assistant_with_content(&saved_content);
        truncated_msg.status = MessageStatus::Truncated;
        if let Some(store) = crate::services::conversation_store::get_store() {
            let _ = store.add_message(conv_id, &truncated_msg).await;
        }
        self.push_message_to_conversation(conv_id, truncated_msg);
        self.sort_conversations_by_updated_at();
    }

    /// 持久化助手消息到存储。
    async fn persist_assistant_message(conv_id: &str, assistant_msg: &Message) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            let _ = store.add_message(conv_id, assistant_msg).await;
        }
    }

    /// 执行记忆后处理：将助手回复与缓存的用户消息配对，聚合写入轮次向量。
    async fn postprocess_turn(conv_id: &str, assistant_msg_id: &str, body: &str) {
        if let Some(pipeline) = crate::services::memory::get_pipeline() {
            if let Err(e) = pipeline.postprocess(conv_id, assistant_msg_id, body).await {
                eprintln!("[xechat] Turn postprocess failed: {}", e);
            }
        }
    }

    /// 重命名对话标题（持久化 + 内存更新）。
    async fn rename_conversation_title(&mut self, conv_id: &str, new_title: &str) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            let _ = store.rename_conversation(conv_id, new_title).await;
        }
        let mut convs = self.conversations.write();
        if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
            conv.title = new_title.to_string();
        }
    }

    /// 处理流式完成：保存助手消息，执行记忆后处理，必要时重命名对话。
    async fn handle_stream_complete(&mut self, conv_id: &str, full_content: &str, full_reasoning: &str, is_first: bool) {
        let (title, body) = if is_first {
            parse_first_response(full_content)
        } else {
            (None, full_content.to_string())
        };

        let mut assistant_msg = Message::new_assistant_with_content(&body);
        if !full_reasoning.is_empty() {
            assistant_msg.reasoning_content = Some(full_reasoning.to_string());
        }
        let assistant_msg_id = assistant_msg.id.clone();

        Self::persist_assistant_message(conv_id, &assistant_msg).await;

        Self::postprocess_turn(conv_id, &assistant_msg_id, &body).await;

        if let Some(new_title) = title {
            self.rename_conversation_title(conv_id, &new_title).await;
        }

        self.push_message_to_conversation(conv_id, assistant_msg);
        self.sort_conversations_by_updated_at();
    }

    /// 处理流式错误：保存失败消息，返回错误显示文本供调用方发送 toast。
    async fn handle_stream_error(&mut self, conv_id: &str, full_content: &str, app_err: AppError) -> String {
        let err_display = app_err.to_string();
        let error_content = if !full_content.is_empty() {
            format!("{}\n\n{}", full_content, err_display)
        } else {
            err_display.clone()
        };
        let mut failed_msg = Message::new_assistant_with_content(&error_content);
        failed_msg.status = MessageStatus::Failed;
        if let Some(store) = crate::services::conversation_store::get_store() {
            let _ = store.add_message(conv_id, &failed_msg).await;
        }
        self.push_message_to_conversation(conv_id, failed_msg);
        self.sort_conversations_by_updated_at();
        err_display
    }

    /// 校验发送前置条件：内容非空、未在流式传输中、有选中对话。
    ///
    /// 返回 `Some(conv_id)` 表示可继续，`None` 表示应中止。
    #[inline]
    pub fn validate_send_prereqs(&self, content: &str) -> Option<String> {
        if content.trim().is_empty() || *self.is_streaming.read() {
            return None;
        }
        self.current_conversation_id.read().clone()
    }

    /// 解析模型提供商配置，同步 Ollama 偏好设置。
    ///
    /// 返回 `Some(provider)` 表示成功，`None` 表示配置缺失（已设置 is_streaming=false）。
    #[inline]
    pub fn resolve_provider(&mut self, config: &XEChatConfig) -> Option<crate::models::config::ModelProvider> {
        let mut provider = config.model_providers.get(&config.model_provider).cloned();
        if provider.is_none() {
            self.is_streaming.set(false);
            return None;
        }
        // 同步 preferences 中的 Ollama 配置到 provider
        if let Some(ref mut p) = provider {
            sync_ollama_host_to_provider(p, config);
        }
        provider
    }

    /// 处理单个流式事件，更新 full_content / full_reasoning。
    ///
    /// 返回 `StreamAction` 指示主循环下一步动作。
    #[inline]
    pub fn handle_stream_event(
        &mut self,
        event: StreamEvent,
        full_content: &mut String,
        full_reasoning: &mut String,
    ) -> StreamAction {
        match event {
            StreamEvent::Chunk(chunk) => {
                full_content.push_str(&chunk);
                self.streaming_content.set(full_content.clone());
                StreamAction::Continue
            }
            StreamEvent::ReasoningChunk(chunk) => {
                full_reasoning.push_str(&chunk);
                self.streaming_reasoning.set(full_reasoning.clone());
                StreamAction::Continue
            }
            StreamEvent::Complete => StreamAction::Complete,
            StreamEvent::Error(app_err) => StreamAction::Error(app_err),
        }
    }

    /// 从 channel 中排空剩余的 Chunk 事件，拼接到 full_content。
    #[inline]
    pub fn drain_remaining_chunks(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<StreamEvent>,
        full_content: &mut String,
    ) {
        while let Ok(event) = rx.try_recv() {
            if let StreamEvent::Chunk(chunk) = event {
                full_content.push_str(&chunk);
            }
        }
    }

    /// 处理从 channel 接收到的事件（Some 或 None），返回循环控制动作。
    #[inline]
    pub async fn handle_received_event(
        &mut self,
        event: Option<StreamEvent>,
        cancel_token: &CancellationToken,
        conv_id: &str,
        full_content: &mut String,
        full_reasoning: &mut String,
        is_first_message: bool,
    ) -> StreamLoopAction {
        match event {
            Some(stream_event) => {
                let action = self.handle_stream_event(stream_event, full_content, full_reasoning);
                self.process_stream_action(action, conv_id, full_content, full_reasoning, is_first_message).await
            }
            None => {
                if cancel_token.is_cancelled() {
                    self.handle_stream_cancel(conv_id, full_content).await;
                }
                StreamLoopAction::Break
            }
        }
    }

    /// 将 StreamAction 转换为 StreamLoopAction，执行对应的完成/错误处理。
    async fn process_stream_action(
        &mut self,
        action: StreamAction,
        conv_id: &str,
        full_content: &mut String,
        full_reasoning: &mut String,
        is_first_message: bool,
    ) -> StreamLoopAction {
        match action {
            StreamAction::Continue => StreamLoopAction::Continue,
            StreamAction::Complete => {
                self.handle_stream_complete(conv_id, full_content, full_reasoning, is_first_message).await;
                StreamLoopAction::Break
            }
            StreamAction::Error(app_err) => {
                let err_display = self.handle_stream_error(conv_id, full_content, app_err).await;
                StreamLoopAction::BreakWithError(err_display)
            }
        }
    }

    /// 构造发送参数并启动流式请求任务，返回事件接收端。
    #[inline]
    pub fn launch_stream_task(
        &mut self,
        provider: crate::models::config::ModelProvider,
        config: &XEChatConfig,
        all_messages: Vec<ChatMessage>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<StreamEvent> {
        let model_config = provider.models.get(&config.model).cloned();
        let temperature = model_config.as_ref().map(|m| m.temperature);
        let top_p = model_config.as_ref().map(|m| m.top_p);
        let client = self.client.clone();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let provider_key_for_route = config.model_provider.clone();
        let provider_key = config.model_provider.clone();
        let params = SendMessageParams {
            provider,
            provider_key,
            model: config.model.clone(),
            messages: all_messages,
            temperature,
            top_p,
            model_config,
        };
        let stream_handle = tokio::spawn(async move { send_message(&client, params, &provider_key_for_route, tx).await });
        self.stream_task.set(Some(stream_handle));
        rx
    }

    /// 清理流式传输状态。
    #[inline]
    pub fn cleanup_streaming_state(&mut self) {
        self.streaming_content.set(String::new());
        self.streaming_reasoning.set(String::new());
        self.is_streaming.set(false);
        self.cancel_token.set(None);
        self.stream_task.set(None);
    }

    /// 向当前对话发送用户消息并启动 AI 流式回复。
    ///
    /// 完整流程：
    /// 1. 校验输入非空且未在流式传输中
    /// 2. 检测临时对话并持久化（首次消息场景）
    /// 3. 追加用户消息到对话并持久化
    /// 4. 构造发送参数（含系统提示注入和上下文压缩）
    /// 5. 通过 `tokio::spawn` 启动异步 HTTP 请求
    /// 6. 监听 `StreamEvent` channel 实时更新 UI 状态
    /// 7. 首次消息时解析标题并重命名对话
    ///
    /// # Arguments
    ///
    /// * `content` - 用户输入的消息文本
    /// * `config` - 应用配置（含模型提供商、参数等）
    /// * `toast_sender` - Toast 通知回调，用于推送错误提示（接收 i18n 翻译后的消息）
    pub async fn send_message(
        &mut self,
        content: String,
        config: XEChatConfig,
        toast_sender: impl FnMut(ToastKind, String) + 'static,
    ) {
        let (conv_id, is_first_message, cancel_for_recv, provider, all_messages) =
            match self.prepare_message_payload(&content, &config).await {
                Some(payload) => payload,
                None => return,
            };

        let mut rx = self.launch_stream_task(provider, &config, all_messages);

        self.run_streaming_loop(&mut rx, &cancel_for_recv, &conv_id, is_first_message, toast_sender).await;

        self.cleanup_streaming_state();
    }

    /// 准备发送消息所需的全部参数：校验、持久化、构建消息列表。
    ///
    /// 返回 `Some((conv_id, is_first_message, cancel_for_recv, provider, all_messages))` 表示准备成功，
    /// `None` 表示应中止发送（校验失败、持久化失败或配置缺失）。
    async fn prepare_message_payload(
        &mut self,
        content: &str,
        config: &XEChatConfig,
    ) -> Option<(String, bool, CancellationToken, crate::models::config::ModelProvider, Vec<ChatMessage>)> {
        let conv_id = self.validate_send_prereqs(content)?;

        self.is_streaming.set(true);
        self.streaming_content.set(String::new());
        self.streaming_reasoning.set(String::new());

        let cancel_token = CancellationToken::new();
        let cancel_for_recv = cancel_token.clone();
        self.cancel_token.set(Some(cancel_token));

        let is_first_message = {
            let convs = self.conversations.read();
            convs.iter().find(|c| c.id == conv_id)
                .map(|c| c.is_temporary)
                .unwrap_or(false)
        };

        let user_msg = Message::new_user(content.to_string());
        self.push_message_to_conversation(&conv_id, user_msg.clone());

        // 缓存用户消息，等待助手回复配对后写入轮次向量
        if let Some(pipeline) = crate::services::memory::get_pipeline() {
            pipeline.on_user_message(&conv_id, &user_msg.id, content);
        }

        if !self.persist_user_message(&conv_id, &user_msg, is_first_message).await {
            self.is_streaming.set(false);
            return None;
        }

        let provider = self.resolve_provider(config)?;
        let all_messages = self.build_chat_messages(content, config, is_first_message).await;

        Some((conv_id, is_first_message, cancel_for_recv, provider, all_messages))
    }

    /// 运行流式接收循环：监听取消信号和流式事件，直到完成/错误/取消。
    ///
    /// 通过 `handle_received_event` 处理每个事件，通过 `drain_remaining_chunks` 排空取消时的剩余数据。
    async fn run_streaming_loop(
        &mut self,
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<StreamEvent>,
        cancel_for_recv: &CancellationToken,
        conv_id: &str,
        is_first_message: bool,
        mut toast_sender: impl FnMut(ToastKind, String),
    ) {
        let mut full_content = String::new();
        let mut full_reasoning = String::new();
        loop {
            tokio::select! {
                // 取消信号：用户点击停止按钮时立即响应
                _ = cancel_for_recv.cancelled() => {
                    // drain channel 中剩余的 chunk
                    Self::drain_remaining_chunks(rx, &mut full_content);
                    self.handle_stream_cancel(conv_id, &full_content).await;
                    break;
                }
                // 流式事件
                event = rx.recv() => {
                    match self.handle_received_event(event, cancel_for_recv, conv_id, &mut full_content, &mut full_reasoning, is_first_message).await {
                        StreamLoopAction::Continue => {}
                        StreamLoopAction::Break => break,
                        StreamLoopAction::BreakWithError(err_display) => {
                            toast_sender(ToastKind::Error, err_display);
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// 流式事件处理动作，由 `handle_stream_event` 返回。
pub enum StreamAction {
    /// 继续接收下一个事件
    Continue,
    /// 收到完成事件
    Complete,
    /// 收到错误事件
    Error(AppError),
}

/// 流式循环控制动作，由 `handle_received_event` 返回。
pub enum StreamLoopAction {
    /// 继续接收下一个事件
    Continue,
    /// 流式处理完成，应退出循环
    Break,
    /// 流式处理出错，应退出循环并发送 Toast
    BreakWithError(String),
}
