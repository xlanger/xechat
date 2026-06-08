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

const FIRST_MESSAGE_SYSTEM_PROMPT: &str = concat!(
    "你是一位智能助手。请遵循以下规则：\n",
    "1. 用户的第一条消息是需要你回答的问题\n",
    "2. 在回复开头，先以 [TITLE:简短标题] 的格式输出一个对话标题（不超过15个字）\n",
    "3. 标题后换行，然后给出对用户的实际回复\n",
    "4. 标题应准确概括用户问题的核心主题"
);

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
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new()
    }
}

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

    /// 加载指定对话的完整内容（含消息），滚动条置底。
    ///
    /// 全记录点击场景：加载所有消息，显示最新 size 条。
    pub async fn load_conversation_content(&mut self, conv_id: &str) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            match store.load_conversation_by_id(conv_id, crate::services::conversation_store::DEFAULT_PAGE_SIZE * 100).await {
                Ok(Some(loaded_conv)) => {
                    let total = loaded_conv.messages.len();
                    let page_size = self.message_pagination.read().page_size;

                    let start = total.saturating_sub(page_size);
                    let end = total;

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
                    if let Some(idx) = convs.iter().position(|c| c.id == conv_id) {
                        convs[idx] = windowed_conv;
                    } else {
                        convs.push(windowed_conv);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("[xechat] load_conversation_content: failed to load conv={}: {}", conv_id, e);
                }
            }
        }
    }

    /// 加载指定对话并定位到特定消息，定位消息置顶。
    ///
    /// 搜索匹配场景：加载定位消息及后续 size-1 条（共 size 条）。
    pub async fn load_conversation_content_anchored(&mut self, conv_id: &str, anchor_msg_id: &str) {
        if let Some(store) = crate::services::conversation_store::get_store() {
            match store.load_conversation_by_id(conv_id, crate::services::conversation_store::DEFAULT_PAGE_SIZE * 100).await {
                Ok(Some(loaded_conv)) => {
                    let total = loaded_conv.messages.len();
                    let page_size = self.message_pagination.read().page_size;

                    let anchor_index = loaded_conv.messages.iter().position(|m| m.id == anchor_msg_id).unwrap_or(0);

                    let start = anchor_index;
                    let end = std::cmp::min(start + page_size, total);

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
                    if let Some(idx) = convs.iter().position(|c| c.id == conv_id) {
                        convs[idx] = windowed_conv;
                    } else {
                        convs.push(windowed_conv);
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("[xechat] load_conversation_content_anchored: failed to load conv={}: {}", conv_id, e);
                }
            }
        }
    }

    /// 向上扩展消息窗口（加载更早的消息）。
    ///
    /// 滚动到顶部时触发，扩展 start_index。
    pub async fn load_more_messages_older(&mut self, conv_id: &str) {
        let (start, page_size, all_len) = {
            let pg = self.message_pagination.read();
            if pg.is_loading || pg.start_index == 0 {
                return;
            }
            (pg.start_index, pg.page_size, pg.all_messages.len())
        };
        if all_len == 0 {
            return;
        }

        self.message_pagination.write().is_loading = true;

        let new_start = start.saturating_sub(page_size);
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
        let (end, page_size, all_len) = {
            let pg = self.message_pagination.read();
            if pg.is_loading || pg.end_index >= pg.all_messages.len() {
                return;
            }
            (pg.end_index, pg.page_size, pg.all_messages.len())
        };
        if all_len == 0 {
            return;
        }

        self.message_pagination.write().is_loading = true;

        let new_end = std::cmp::min(end + page_size, all_len);
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

        let e5 = match tokio::task::spawn_blocking(
            crate::services::embedder::e5::E5Embedder::new,
        )
        .await
        {
            Ok(Ok(e5)) => std::sync::Arc::new(e5),
            Ok(Err(e)) => {
                eprintln!("[xechat] Failed to init E5 embedder: {}", e);
                return;
            }
            Err(e) => {
                eprintln!("[xechat] E5 embedder task panicked: {}", e);
                return;
            }
        };

        let manager = std::sync::Arc::new(
            crate::services::embedder::EmbedManager::new(e5)
        );

        if config.preferences.embed_provider == "ollama" && !config.preferences.ollama.embed_model.is_empty() {
            let ollama_host = if config.preferences.ollama.host.is_empty() {
                "http://localhost:11434"
            } else {
                &config.preferences.ollama.host
            };
            match crate::services::ollama::embed::OllamaEmbedder::probe(
                ollama_host,
                &config.preferences.ollama.embed_model,
            )
            .await
            {
                Ok(ollama) => {
                    eprintln!(
                        "[xechat] Ollama embedder ready: {} (dim={})",
                        config.preferences.ollama.embed_model,
                        ollama.dimension()
                    );
                    if let Err(e) = manager.enable_ollama(std::sync::Arc::new(ollama)) {
                        eprintln!("[xechat] Failed to enable Ollama: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[xechat] Ollama probe failed ({}): {}, using E5 only",
                        config.preferences.ollama.embed_model, e
                    );
                }
            }
        }

        let embedder: std::sync::Arc<dyn crate::services::embedder::Embedder> = manager;

        if let Err(e) = crate::services::embedder::init_embedder(embedder) {
            eprintln!("[xechat] Embedder init error: {}", e);
        }

        let conv_lancedb_path = crate::services::paths::get_lancedb_path();
        std::fs::create_dir_all(&conv_lancedb_path).ok();
        let conv_lancedb_str = conv_lancedb_path.to_str().unwrap_or("").to_string();

        // 初始化向量存储（turns 表）
        let vector_store: std::sync::Arc<dyn crate::services::vector_store::VectorStore> = {
            match crate::services::vector_store::lancedb_store::LanceDbStore::open(&conv_lancedb_str).await {
                Ok(mut vs) => {
                    if let Err(e) = vs.ensure_table().await {
                        eprintln!("[xechat] Failed to ensure turns table: {}", e);
                    }
                    std::sync::Arc::new(vs)
                }
                Err(e) => {
                    eprintln!("[xechat] Failed to open LanceDbStore for turns: {}", e);
                    return;
                }
            }
        };

        // 初始化记忆管线（需要 embedder + vector_store）
        if let Some(embedder) = crate::services::embedder::get_embedder() {
            let pipeline = crate::services::memory::MemoryPipeline::new(
                embedder,
                vector_store.clone(),
            );
            if let Err(e) = crate::services::memory::init_pipeline(pipeline) {
                eprintln!("[xechat] Failed to init memory pipeline: {}", e);
            }
        }

        match crate::services::conversation_store::ConversationStore::open(&conv_lancedb_str, vector_store).await {
            Ok(mut store) => {
                if let Err(e) = store.ensure_table().await {
                    eprintln!("[xechat] Failed to ensure conversations table: {}", e);
                }
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
        mut toast_sender: impl FnMut(ToastKind, String) + 'static,
    ) {
        if content.trim().is_empty() || *self.is_streaming.read() {
            return;
        }

        let conv_id = match self.current_conversation_id.read().clone() {
            Some(id) => id,
            None => {
                return;
            }
        };

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

        let user_msg = Message::new_user(content.clone());

        {
            let mut convs = self.conversations.write();
            if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
                conv.messages.push(user_msg.clone());
                conv.updated_at = chrono::Utc::now();
            }
        }

        // 缓存用户消息，等待助手回复配对后写入轮次向量
        if let Some(pipeline) = crate::services::memory::get_pipeline() {
            pipeline.on_user_message(&conv_id, &user_msg.id, &content);
        }

        if is_first_message {
            let new_conv = Conversation {
                id: conv_id.clone(),
                title: String::from("New Chat"),
                messages: vec![user_msg.clone()],
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                is_temporary: false,
            };
            let save_result = if let Some(store) = crate::services::conversation_store::get_store() {
                store.save_conversation(&new_conv).await
                    .map_err(|e| e.to_string())
            } else {
                Err("ConversationStore not initialized".to_string())
            };
            if let Err(e) = save_result {
                eprintln!("[xechat] Failed to create conversation: {}", e);
                self.is_streaming.set(false);
                return;
            }
            let mut convs = self.conversations.write();
            if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id) {
                conv.is_temporary = false;
            }
        } else {
            if let Some(store) = crate::services::conversation_store::get_store() {
                let _ = store.add_message(&conv_id, &user_msg).await;
            }
        }

        let mut provider = match config.model_providers.get(&config.model_provider) {
            Some(p) => p.clone(),
            None => {
                self.is_streaming.set(false);
                return;
            }
        };

        // 同步 preferences 中的 Ollama 配置到 provider
        if config.model_provider == "ollama" {
            if !config.preferences.ollama.host.is_empty() {
                provider.base_url = config.preferences.ollama.host.clone();
            }
        }

        let memory_prepend: Vec<ChatMessage> = if let Some(pipeline) = crate::services::memory::get_pipeline() {
            let recent_msgs: Vec<crate::Message> = self.selected_conversation()
                .map(|c| c.messages.clone())
                .unwrap_or_default();
            let preprocess_result = pipeline.preprocess(&content, &recent_msgs).await;
            if preprocess_result.memory_used {
                preprocess_result.enhanced_messages
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let all_messages: Vec<ChatMessage> = {
            let mut msgs: Vec<ChatMessage> = vec![];

            if !memory_prepend.is_empty() {
                msgs.extend(memory_prepend);
            }

            if is_first_message {
                msgs.push(ChatMessage {
                    role: "system".into(),
                    content: FIRST_MESSAGE_SYSTEM_PROMPT.to_string(),
                });
            }

            if let Some(conv) = self.selected_conversation() {
                let history: Vec<_> = conv.messages.iter()
                    .filter(|m| m.role == MessageRole::User || (!m.content.is_empty() && m.role == MessageRole::Assistant))
                    .map(|m| ChatMessage {
                        role: if m.role == MessageRole::User { "user".into() } else { "assistant".into() },
                        content: m.content.clone(),
                    }).collect();
                msgs.extend(history);
            }

            let max_tokens = config.max_context_tokens.unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS);
            let auto_management = config.auto_context_management.unwrap_or(DEFAULT_AUTO_CONTEXT_MANAGEMENT);
            compress_messages(&msgs, max_tokens, auto_management)
        };

        let model_config = provider.models.get(&config.model).cloned();
        let temperature = model_config.as_ref().map(|m| m.temperature);
        let top_p = model_config.as_ref().map(|m| m.top_p);
        let client = self.client.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

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
        let conv_id_clone = conv_id.clone();
        let stream_handle = tokio::spawn(async move { send_message(&client, params, &provider_key_for_route, tx).await });
        self.stream_task.set(Some(stream_handle));

        let mut full_content = String::new();
        let mut full_reasoning = String::new();
        loop {
            tokio::select! {
                // 取消信号：用户点击停止按钮时立即响应
                _ = cancel_for_recv.cancelled() => {
                    // drain channel 中剩余的 chunk
                    while let Ok(event) = rx.try_recv() {
                        if let StreamEvent::Chunk(chunk) = event {
                            full_content.push_str(&chunk);
                        }
                    }
                    // 使用 streaming_content（UI 已显示的内容）作为截断保存的数据源，
                    // 因为 cancel 信号可能在 chunk 到达 channel 之前就被 select! 捕获
                    let saved_content = if !full_content.is_empty() {
                        full_content.clone()
                    } else {
                        self.streaming_content.read().clone()
                    };
                    if !saved_content.is_empty() {
                        let mut truncated_msg = Message::new_assistant_with_content(&saved_content);
                        truncated_msg.status = MessageStatus::Truncated;
                        if let Some(store) = crate::services::conversation_store::get_store() {
                            let _ = store.add_message(&conv_id_clone, &truncated_msg).await;
                        }
                        {
                            let mut convs = self.conversations.write();
                            if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id_clone) {
                                conv.messages.push(truncated_msg);
                                conv.updated_at = chrono::Utc::now();
                            }
                        }
                        {
                            let mut convs = self.conversations.write();
                            convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                        }
                    }
                    break;
                }
                // 流式事件
                event = rx.recv() => {
                    match event {
                        Some(StreamEvent::Chunk(chunk)) => {
                            full_content.push_str(&chunk);
                            self.streaming_content.set(full_content.clone());
                        }
                        Some(StreamEvent::ReasoningChunk(chunk)) => {
                            full_reasoning.push_str(&chunk);
                            self.streaming_reasoning.set(full_reasoning.clone());
                        }
                        Some(StreamEvent::Complete) => {
                            let (title, body) = if is_first_message {
                                parse_first_response(&full_content)
                            } else {
                                (None, full_content.clone())
                            };

                            let mut assistant_msg = Message::new_assistant_with_content(&body);
                            if !full_reasoning.is_empty() {
                                assistant_msg.reasoning_content = Some(full_reasoning.clone());
                            }
                            let assistant_msg_id = assistant_msg.id.clone();
                            if let Some(store) = crate::services::conversation_store::get_store() {
                                let _ = store.add_message(&conv_id_clone, &assistant_msg).await;
                            }

                            // 助手回复完成后，与缓存的用户消息配对，聚合写入轮次向量
                            if let Some(pipeline) = crate::services::memory::get_pipeline() {
                                if let Err(e) = pipeline.postprocess(
                                    &conv_id_clone,
                                    &assistant_msg_id,
                                    &body,
                                ).await {
                                    eprintln!("[xechat] Turn postprocess failed: {}", e);
                                }
                            }

                            if let Some(new_title) = title {
                                if let Some(store) = crate::services::conversation_store::get_store() {
                                    let _ = store.rename_conversation(&conv_id_clone, &new_title).await;
                                }
                                let mut convs = self.conversations.write();
                                if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id_clone) {
                                    conv.title = new_title;
                                }
                            }

                            {
                                let mut convs = self.conversations.write();
                                if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id_clone) {
                                    conv.messages.push(assistant_msg);
                                    conv.updated_at = chrono::Utc::now();
                                }
                            }

                            {
                                let mut convs = self.conversations.write();
                                convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                            }
                            break;
                        }
                        Some(StreamEvent::Error(app_err)) => {
                            let err_display = app_err.to_string();

                            let error_content = if !full_content.is_empty() {
                                format!("{}\n\n{}", full_content, err_display)
                            } else {
                                err_display.clone()
                            };
                            let mut failed_msg = Message::new_assistant_with_content(&error_content);
                            failed_msg.status = MessageStatus::Failed;
                            if let Some(store) = crate::services::conversation_store::get_store() {
                                let _ = store.add_message(&conv_id_clone, &failed_msg).await;
                            }

                            {
                                let mut convs = self.conversations.write();
                                if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id_clone) {
                                    conv.messages.push(failed_msg);
                                    conv.updated_at = chrono::Utc::now();
                                }
                            }

                            {
                                let mut convs = self.conversations.write();
                                convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                            }

                            toast_sender(ToastKind::Error, err_display);
                            break;
                        }
                        None => {
                            // channel 关闭，流式任务结束
                            // 如果是用户取消导致的关闭，保存已生成内容为截断消息
                            if cancel_for_recv.is_cancelled() {
                                let saved_content = if !full_content.is_empty() {
                                    full_content.clone()
                                } else {
                                    self.streaming_content.read().clone()
                                };
                                if !saved_content.is_empty() {
                                    let mut truncated_msg = Message::new_assistant_with_content(&saved_content);
                                    truncated_msg.status = MessageStatus::Truncated;
                                    if let Some(store) = crate::services::conversation_store::get_store() {
                                        let _ = store.add_message(&conv_id_clone, &truncated_msg).await;
                                    }
                                    {
                                        let mut convs = self.conversations.write();
                                        if let Some(conv) = convs.iter_mut().find(|c| c.id == conv_id_clone) {
                                            conv.messages.push(truncated_msg);
                                            conv.updated_at = chrono::Utc::now();
                                        }
                                    }
                                    {
                                        let mut convs = self.conversations.write();
                                        convs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        // 统一清理流式状态
        self.streaming_content.set(String::new());
        self.streaming_reasoning.set(String::new());
        self.is_streaming.set(false);
        self.cancel_token.set(None);
        self.stream_task.set(None);
    }
}
