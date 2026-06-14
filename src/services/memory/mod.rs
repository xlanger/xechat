use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::Mutex;

use crate::Message;
use crate::models::ai::ChatMessage;
use crate::services::embedder::Embedder;
use crate::services::intent::BuiltinIntentAnalyzer;
use rust_i18n::t;
use crate::services::vector_store::VectorStore;

const MEMORY_SYSTEM_PROMPT: &str = "以下是与用户问题相关的历史记忆：\n";
const MEMORY_FOOTER: &str = "\n\n请结合以上记忆上下文回答用户的问题。";
const MAX_MEMORY_RESULTS: usize = 5;

pub struct PreprocessResult {
    pub enhanced_messages: Vec<ChatMessage>,
    pub memory_used: bool,
}

pub struct MemoryPipeline {
    embedder: Arc<dyn Embedder>,
    intent_analyzer: BuiltinIntentAnalyzer,
    vector_store: Arc<dyn VectorStore>,
    /// 缓存等待配对的用户消息（conversation_id → (message_id, content)）
    pending_user: Mutex<HashMap<String, (String, String)>>,
}

impl MemoryPipeline {
    pub fn new(
        embedder: Arc<dyn Embedder>,
        vector_store: Arc<dyn VectorStore>,
    ) -> Self {
        Self {
            embedder,
            intent_analyzer: BuiltinIntentAnalyzer::new(),
            vector_store,
            pending_user: Mutex::new(HashMap::new()),
        }
    }

    /// 缓存用户消息，等待助手回复配对后写入。
    pub fn on_user_message(&self, conv_id: &str, msg_id: &str, content: &str) {
        if let Ok(mut pending) = self.pending_user.lock() {
            pending.insert(conv_id.to_string(), (msg_id.to_string(), content.to_string()));
        }
    }

    pub async fn preprocess(&self, user_input: &str, recent_messages: &[Message]) -> PreprocessResult {
        let intent = self.intent_analyzer.analyze(user_input, recent_messages);

        if !intent.needs_memory {
            return PreprocessResult {
                enhanced_messages: Vec::new(),
                memory_used: false,
            };
        }

        let query_vector = match self.embedder.encode_query(&intent.memory_query).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[xechat] Embed query failed: {}", e);
                return PreprocessResult {
                    enhanced_messages: Vec::new(),
                    memory_used: false,
                };
            }
        };

        let hits = self.vector_store.search_turns(&query_vector, MAX_MEMORY_RESULTS)
            .await
            .unwrap_or_default();

        // 按 conversation_id 去重，保留最高分
        let mut seen_conv: HashSet<String> = HashSet::new();
        let hits: Vec<crate::models::memory::SearchHit> = hits
            .into_iter()
            .filter(|hit| seen_conv.insert(hit.conversation_id.clone()))
            .collect();

        if hits.is_empty() {
            return PreprocessResult {
                enhanced_messages: Vec::new(),
                memory_used: false,
            };
        }

        let mut memory_text = MEMORY_SYSTEM_PROMPT.to_string();
        for (i, hit) in hits.iter().enumerate() {
            let qa_text = if !hit.user_content.is_empty() {
                format!("{}：{}\n\n{}：{}",
                    t!("search.question"), hit.user_content,
                    t!("search.answer"), hit.content)
            } else {
                hit.content.clone()
            };
            let snippet = if qa_text.len() > 100 {
                let end = qa_text.char_indices()
                    .take_while(|(idx, _)| *idx < 100)
                    .last()
                    .map(|(idx, c)| idx + c.len_utf8())
                    .unwrap_or(0);
                format!("{}...", &qa_text[..end])
            } else if qa_text.is_empty() {
                format!("记忆片段 {}", hit.entry_id)
            } else {
                qa_text
            };
            memory_text.push_str(&format!(
                "{}. [相关度 {:.0}%] {}\n",
                i + 1,
                hit.score * 100.0,
                snippet
            ));
        }
        memory_text.push_str(MEMORY_FOOTER);

        let enhanced_messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: memory_text,
            },
        ];

        PreprocessResult {
            enhanced_messages,
            memory_used: true,
        }
    }

    /// 助手回复完成后，与缓存的用户消息配对，聚合写入轮次向量。
    pub async fn postprocess(
        &self,
        conv_id: &str,
        assistant_msg_id: &str,
        assistant_content: &str,
    ) -> anyhow::Result<()> {
        // 取出缓存的用户消息
        let user_info = if let Ok(mut pending) = self.pending_user.lock() {
            pending.remove(conv_id)
        } else {
            None
        };

        let (user_msg_id, user_content) = user_info.unwrap_or_default();

        // 合并轮次文本
        let turn_text = format!("用户：{}\n助手：{}", user_content, assistant_content);

        // 编码轮次文本
        let char_count = turn_text.chars().count();
        let chunk_params = crate::services::embedder::ChunkParams::from_context_window(
            self.embedder.context_window()
        );
        let chunks = if char_count < chunk_params.target_chars {
            // 短文本：整条编码
            let embedding = self.embedder.encode_passage(&turn_text).await?;
            vec![crate::models::memory::ChunkMeta {
                chunk_index: 0,
                chunk_text: turn_text.clone(),
                start_char: 0,
                end_char: turn_text.len() as u32,
                embedding,
            }]
        } else {
            // 长文本：语义分块编码
            let spans = crate::services::embedder::manager::semantic_chunk(&turn_text, chunk_params);
            let mut chunk_metas = Vec::with_capacity(spans.len());
            for (i, span) in spans.iter().enumerate() {
                let embedding = self.embedder.encode_passage(&span.text).await?;
                chunk_metas.push(crate::models::memory::ChunkMeta {
                    chunk_index: i as u32,
                    chunk_text: span.text.clone(),
                    start_char: span.start as u32,
                    end_char: span.end as u32,
                    embedding,
                });
            }
            chunk_metas
        };

        let entry = crate::models::memory::TurnEntry {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conv_id.to_string(),
            user_message_id: user_msg_id,
            assistant_message_id: assistant_msg_id.to_string(),
            turn_index: 0,
            user_content,
            assistant_content: assistant_content.to_string(),
            timestamp: chrono::Utc::now(),
            chunks,
        };

        self.vector_store.add_turn(entry).await?;
        Ok(())
    }
}

static PIPELINE: std::sync::RwLock<Option<Arc<MemoryPipeline>>> = std::sync::RwLock::new(None);

pub fn init_pipeline(pipeline: MemoryPipeline) -> anyhow::Result<()> {
    let mut guard = PIPELINE.write().map_err(|e| anyhow::anyhow!("Pipeline lock poisoned: {}", e))?;
    *guard = Some(Arc::new(pipeline));
    Ok(())
}

pub fn get_pipeline() -> Option<Arc<MemoryPipeline>> {
    PIPELINE.read().ok().and_then(|guard| guard.as_ref().cloned())
}
