use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::PathBuf;
use crate::{Conversation, Message, MessageStatus};
use crate::services::paths;

#[cfg_attr(test, mockall::automock)]
pub trait ConversationService {
    fn load_conversations(&self) -> Result<Vec<Conversation>, String>;
    fn load_conversation_list(&self) -> Result<Vec<Conversation>, String>;
    fn load_conversation_by_id(&self, conv_id: &str) -> Result<Option<Conversation>, String>;
    fn save_conversation(&self, conversation: &Conversation) -> Result<(), String>;
    fn create_conversation(&self, title: &str) -> Result<Conversation, String>;
    fn add_message_to_conversation(&self, conv_id: &str, message: &Message) -> Result<(), String>;
    fn update_message_content(&self, conv_id: &str, msg_id: &str, new_content: &str) -> Result<(), String>;
    fn rename_conversation(&self, conv_id: &str, new_title: &str) -> Result<(), String>;
    fn delete_conversation(&self, conv_id: &str) -> Result<(), String>;
    fn conversation_exists(&self, conv_id: &str) -> bool;
    fn update_last_message(&self, conv_id: &str, content: &str, status: MessageStatus) -> Result<(), String>;
    fn remove_last_message(&self, conv_id: &str) -> Result<(), String>;
}

pub struct FileConversationService;

impl FileConversationService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileConversationService {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationService for FileConversationService {
    fn load_conversations(&self) -> Result<Vec<Conversation>, String> {
        load_conversations()
    }

    fn load_conversation_list(&self) -> Result<Vec<Conversation>, String> {
        load_conversation_list()
    }

    fn load_conversation_by_id(&self, conv_id: &str) -> Result<Option<Conversation>, String> {
        load_conversation_by_id(conv_id)
    }

    fn save_conversation(&self, conversation: &Conversation) -> Result<(), String> {
        save_conversation(conversation)
    }

    fn create_conversation(&self, title: &str) -> Result<Conversation, String> {
        create_conversation(title)
    }

    fn add_message_to_conversation(&self, conv_id: &str, message: &Message) -> Result<(), String> {
        add_message_to_conversation(conv_id, message)
    }

    fn update_message_content(&self, conv_id: &str, msg_id: &str, new_content: &str) -> Result<(), String> {
        update_message_content(conv_id, msg_id, new_content)
    }

    fn rename_conversation(&self, conv_id: &str, new_title: &str) -> Result<(), String> {
        rename_conversation(conv_id, new_title)
    }

    fn delete_conversation(&self, conv_id: &str) -> Result<(), String> {
        delete_conversation(conv_id)
    }

    fn conversation_exists(&self, conv_id: &str) -> bool {
        conversation_exists(conv_id)
    }

    fn update_last_message(&self, conv_id: &str, content: &str, status: MessageStatus) -> Result<(), String> {
        update_last_message(conv_id, content, status)
    }

    fn remove_last_message(&self, conv_id: &str) -> Result<(), String> {
        remove_last_message(conv_id)
    }
}

/// 从磁盘读取并解析对话文件。
///
/// 文件存在时返回 `Ok(Some(Conversation))`，文件不存在返回 `Ok(None)`，
/// 读取或解析失败时返回 `Err`。
pub fn read_conversation_from_file(file_path: &std::path::Path) -> Result<Option<Conversation>, String> {
    if !file_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read conversation: {}", e))?;
    let conv = serde_json::from_str::<Conversation>(&content)
        .map_err(|e| format!("Failed to parse conversation: {}", e))?;
    Ok(Some(conv))
}

/// 读取对话文件，失败时返回带默认值的对话。
///
/// 文件存在且解析成功时返回解析结果，否则返回包含 id 和 title 的默认对话。
pub fn read_conversation_or_default(id: &str, title: &str) -> Conversation {
    let file_path = paths::get_conversation_file(id);
    if let Ok(Some(conv)) = read_conversation_from_file(&file_path) {
        return conv;
    }
    Conversation {
        id: id.to_string(),
        title: title.to_string(),
        messages: Vec::new(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_temporary: false,
    }
}

/// 读取对话文件并反序列化为可变 Conversation。
///
/// 文件不存在时返回 `Err("Conversation not found")`。
pub fn load_conversation_mut(conv_id: &str) -> Result<Conversation, String> {
    let file_path = paths::get_conversation_file(conv_id);
    match read_conversation_from_file(&file_path) {
        Ok(Some(conv)) => Ok(conv),
        Ok(None) => Err("Conversation not found".into()),
        Err(e) => Err(e),
    }
}

pub fn write_file(path: &PathBuf, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
    }
    std::fs::write(path, content).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

pub fn load_conversations_index() -> Result<HashMap<String, String>, String> {
    paths::ensure_app_dir()?;
    let path = paths::get_conversations_index_path();
    let mut index = if !path.exists() {
        HashMap::new()
    } else {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read conversations index: {}", e))?;
        if content.trim().is_empty() {
            HashMap::new()
        } else {
            serde_json::from_str(&content).unwrap_or_else(|e| {
                eprintln!("[xechat] Corrupted conversations index, resetting: {}", e);
                HashMap::new()
            })
        }
    };

    // Auto-recovery: if index is empty but conversation files exist, rebuild index
    if index.is_empty() {
        let conv_dir = paths::get_app_dir().join("conversations");
        if conv_dir.exists() {
            let mut recovered = false;
            if let Ok(entries) = std::fs::read_dir(&conv_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let conv_id = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_string();
                        if !conv_id.is_empty() {
                            let conv_file = path.join("conversation.json");
                            if conv_file.exists()
                                && let Ok(content) = std::fs::read_to_string(&conv_file)
                                    && let Ok(conv) = serde_json::from_str::<Conversation>(&content) {
                                        index.insert(conv_id, conv.title);
                                        recovered = true;
                                    }
                        }
                    }
                }
            }
            if recovered {
                eprintln!("[xechat] Recovered {} conversations from disk", index.len());
                let _ = save_conversations_index(&index);
            }
        }
    }

    Ok(index)
}

fn save_conversations_index(index: &HashMap<String, String>) -> Result<(), String> {
    paths::ensure_app_dir()?;
    let json = serde_json::to_string_pretty(index)
        .map_err(|e| format!("Failed to serialize index: {}", e))?;
    write_file(&paths::get_conversations_index_path(), &json)
}

/// 加载所有对话及其完整消息内容。
///
/// 从索引文件中读取对话列表，
/// 逐个加载每个对话的完整消息数据。结果按 `updated_at` 降序排列。
///
/// # Returns
///
/// 成功返回所有对话的列表，含完整消息。
///
/// # Errors
///
/// 索引文件读取或 JSON 解析失败时返回错误描述字符串。
/// 个别对话文件损坏时会跳过并打印警告，不影响整体返回。
pub fn load_conversations() -> Result<Vec<Conversation>, String> {
    let index = load_conversations_index()?;
    let mut conversations: Vec<Conversation> = Vec::new();

    for (id, title) in &index {
        let file_path = paths::get_conversation_file(id);
        if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    match serde_json::from_str::<Conversation>(&content) {
                        Ok(conv) => conversations.push(conv),
                        Err(e) => eprintln!("[xechat] Skipping corrupted conversation {}: {}", id, e),
                    }
                }
                Err(e) => eprintln!("[xechat] Failed to read conversation {}: {}", id, e),
            }
        } else {
            conversations.push(Conversation {
                id: id.clone(),
                title: title.clone(),
                messages: Vec::new(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                is_temporary: false,
            });
        }
    }

    conversations.sort_by_key(|b| Reverse(b.updated_at));
    Ok(conversations)
}

/// 加载对话列表，包含消息数量信息。
///
/// 为保留 `messages.len()` 的准确性，直接使用反序列化得到的对话数据。
/// 分页场景下每页数据量有限，完整加载不会造成性能问题。
pub fn load_conversation_list() -> Result<Vec<Conversation>, String> {
    let index = load_conversations_index()?;
    let mut conversations = Vec::new();

    for (id, _title) in &index {
        conversations.push(read_conversation_or_default(id, _title));
    }

    conversations.sort_by_key(|c| Reverse(c.updated_at));
    Ok(conversations)
}

/// 分页加载对话列表，按 `updated_at` 降序排列。
///
/// 仅返回当前页的对话数据，不加载消息内容。
///
/// # Arguments
///
/// * `page` - 页码（从 1 开始）
/// * `page_size` - 每页条数
///
/// # Returns
///
/// 返回元组 `(当前页对话列表, 总条数)`
pub fn load_conversation_list_paged(page: usize, page_size: usize) -> Result<(Vec<Conversation>, usize), String> {
    let all = load_conversation_list()?;
    let total = all.len();
    let start = (page.saturating_sub(1)) * page_size;
    let end = std::cmp::min(start + page_size, total);
    let page_items = if start < total {
        all[start..end].to_vec()
    } else {
        Vec::new()
    };
    Ok((page_items, total))
}

/// 根据 ID 加载单个对话的完整数据。
///
/// # Arguments
///
/// * `conv_id` - 对话唯一标识符
///
/// # Returns
///
/// 成功返回 `Some(Conversation)`，对话文件不存在返回 `None`。
///
/// # Errors
///
/// 文件读取或 JSON 解析失败时返回错误描述字符串。
pub fn load_conversation_by_id(conv_id: &str) -> Result<Option<Conversation>, String> {
    let file_path = paths::get_conversation_file(conv_id);
    if !file_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read conversation: {}", e))?;
    let conv = serde_json::from_str::<Conversation>(&content)
        .map_err(|e| format!("Failed to parse conversation: {}", e))?;
    Ok(Some(conv))
}

/// 持久化保存对话数据。
///
/// 将对话序列化为 JSON 写入对应文件，并同步更新对话索引。
///
/// # Arguments
///
/// * `conversation` - 待保存的对话对象引用
///
/// # Errors
///
/// 序列化失败、文件写入失败或索引更新失败时返回错误描述字符串。
pub fn save_conversation(conversation: &Conversation) -> Result<(), String> {
    let file_path = paths::get_conversation_file(&conversation.id);
    let json = serde_json::to_string_pretty(conversation)
        .map_err(|e| format!("Failed to serialize conversation: {}", e))?;
    write_file(&file_path, &json)?;

    let mut index = load_conversations_index().unwrap_or_default();
    index.insert(conversation.id.clone(), conversation.title.clone());
    save_conversations_index(&index)?;

    Ok(())
}

/// 创建新对话并保存到磁盘。
///
/// # Arguments
///
/// * `title` - 对话标题
///
/// # Returns
///
/// 创建成功返回完整的 [`Conversation`] 对象。
///
/// # Errors
///
/// 保存失败时返回错误描述字符串。
pub fn create_conversation(title: &str) -> Result<Conversation, String> {
    let conv = Conversation::new(title.to_string());
    save_conversation(&conv)?;
    Ok(conv)
}

/// 向指定对话追加一条消息。
///
/// 读取对话文件，追加消息后更新 `updated_at` 时间戳并保存。
///
/// # Arguments
///
/// * `conv_id` - 目标对话 ID
/// * `message` - 待追加的消息对象引用
///
/// # Errors
///
/// 对话不存在、文件读取/解析/写入失败时返回错误描述字符串。
pub fn add_message_to_conversation(conv_id: &str, message: &Message) -> Result<(), String> {
    let mut conversation = load_conversation_mut(conv_id)?;

    conversation.messages.push(message.clone());
    conversation.updated_at = chrono::Utc::now();
    save_conversation(&conversation)?;
    Ok(())
}

/// 更新对话中指定消息的文本内容。
///
/// 按消息 ID 查找并替换 content 字段，更新 `updated_at` 后保存。
///
/// # Arguments
///
/// * `conv_id` - 目标对话 ID
/// * `msg_id` - 目标消息 ID
/// * `new_content` - 替换后的文本内容
///
/// # Errors
///
/// 对话文件不存在、读取/解析/写入失败时返回错误描述字符串。
/// 若未找到匹配的消息 ID 则静默跳过（不报错）。
pub fn update_message_content(conv_id: &str, msg_id: &str, new_content: &str) -> Result<(), String> {
    let mut conversation = load_conversation_mut(conv_id)?;

    if let Some(msg) = conversation.messages.iter_mut().find(|m| m.id == msg_id) {
        msg.content = new_content.to_string();
        conversation.updated_at = chrono::Utc::now();
        save_conversation(&conversation)?;
    }
    Ok(())
}

/// 重命名指定对话的标题。
///
/// # Arguments
///
/// * `conv_id` - 目标对话 ID
/// * `new_title` - 新标题文本
///
/// # Errors
///
/// 对话不存在、文件读取/解析/写入失败时返回错误描述字符串。
pub fn rename_conversation(conv_id: &str, new_title: &str) -> Result<(), String> {
    let mut conversation = load_conversation_mut(conv_id)?;

    conversation.title = new_title.to_string();
    conversation.updated_at = chrono::Utc::now();
    save_conversation(&conversation)
}

/// 删除指定对话及其所有关联文件。
///
/// 删除对话目录、对话 JSON 文件，并从索引中移除条目。
///
/// # Arguments
///
/// * `conv_id` - 待删除的对话 ID
///
/// # Errors
///
/// 目录删除或索引更新失败时返回错误描述字符串。
pub fn delete_conversation(conv_id: &str) -> Result<(), String> {
    let conv_dir = paths::get_conversation_dir(conv_id);
    if conv_dir.exists() {
        std::fs::remove_dir_all(&conv_dir)
            .map_err(|e| format!("Failed to delete conversation dir: {}", e))?;
    }

    let file_path = paths::get_conversation_file(conv_id);
    if file_path.exists() {
        let _ = std::fs::remove_file(&file_path);
    }

    let mut index = load_conversations_index().unwrap_or_default();
    index.remove(conv_id);
    save_conversations_index(&index)?;

    Ok(())
}

/// 检查指定对话是否存在。
///
/// # Arguments
///
/// * `conv_id` - 对话 ID
///
/// # Returns
///
/// 对话文件存在则返回 `true`，否则返回 `false`。
pub fn conversation_exists(conv_id: &str) -> bool {
    paths::get_conversation_file(conv_id).exists()
}

/// 更新对话最后一条消息的内容和状态。
///
/// 通常用于流式回复中逐片更新 assistant 消息的 content 和
/// `MessageStatus`。
///
/// # Arguments
///
/// * `conv_id` - 目标对话 ID
/// * `content` - 新的消息文本内容
/// * `status` - 更新后的消息状态
///
/// # Errors
///
/// 对话不存在、文件读取/解析/写入失败时返回错误描述字符串。
/// 若消息列表为空则静默跳过。
pub fn update_last_message(conv_id: &str, content: &str, status: crate::MessageStatus) -> Result<(), String> {
    let mut conversation = load_conversation_mut(conv_id)?;

    if let Some(msg) = conversation.messages.last_mut() {
        msg.content = content.to_string();
        msg.status = status;
        conversation.updated_at = chrono::Utc::now();
        save_conversation(&conversation)?;
    }
    Ok(())
}

/// 移除对话的最后一条消息。
///
/// 通常用于用户撤销或流式生成中断时回退 assistant 消息。
///
/// # Arguments
///
/// * `conv_id` - 目标对话 ID
///
/// # Errors
///
/// 对话不存在、文件读取/解析/写入失败时返回错误描述字符串。
pub fn remove_last_message(conv_id: &str) -> Result<(), String> {
    let mut conversation = load_conversation_mut(conv_id)?;

    conversation.messages.pop();
    conversation.updated_at = chrono::Utc::now();
    save_conversation(&conversation)?;
    Ok(())
}
