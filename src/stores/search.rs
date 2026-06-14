//! 搜索状态管理 Store。
//!
//! 持有搜索查询、结果列表、搜索类型、选中结果等搜索核心状态，
//! 提供查询设置、结果选择、搜索执行等业务方法。
//! 另外管理"最近对话"分页状态，空查询时从数据库分页加载对话列表。
//! 本模块属于 stores 层，依赖 `crate::models::memory` 提供的数据类型，
//! 以及 `crate::services::search` 提供的全文搜索和混合搜索功能。

use dioxus::prelude::*;
use crate::models::memory::{SearchResult, SearchType};
use crate::services::conversation_store::{ConversationSummary, SEARCH_PAGE_SIZE};

#[derive(Clone)]
pub struct SearchStore {
    pub query: Signal<String>,
    pub results: Signal<Vec<SearchResult>>,
    pub search_type: Signal<SearchType>,
    pub is_searching: Signal<bool>,
    pub selected_result: Signal<Option<SearchResult>>,
    pub preview_conversation_id: Signal<Option<String>>,
    pub highlight_message_id: Signal<Option<String>>,
    pub recent_page: Signal<usize>,
    pub recent_page_size: Signal<usize>,
    pub recent_items: Signal<Vec<ConversationSummary>>,
    pub recent_total: Signal<usize>,
}

impl Default for SearchStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchStore {
    pub fn new() -> Self {
        Self {
            query: Signal::new(String::new()),
            results: Signal::new(Vec::new()),
            search_type: Signal::new(SearchType::Hybrid),
            is_searching: Signal::new(false),
            selected_result: Signal::new(None),
            preview_conversation_id: Signal::new(None),
            highlight_message_id: Signal::new(None),
            recent_page: Signal::new(1),
            recent_page_size: Signal::new(SEARCH_PAGE_SIZE),
            recent_items: Signal::new(Vec::new()),
            recent_total: Signal::new(0),
        }
    }

    pub fn set_query(&mut self, query: String) {
        self.query.set(query);
    }

    pub fn set_search_type(&mut self, search_type: SearchType) {
        self.search_type.set(search_type);
    }

    pub fn select_result(&mut self, result: SearchResult) {
        let conv_id = result.conversation_id.clone();
        let msg_id = result.message_id.clone();
        self.selected_result.set(Some(result));
        self.preview_conversation_id.set(Some(conv_id));
        // message_id 非空时为搜索匹配场景，设置高亮定位
        // message_id 为空时为全记录点击场景，不设置高亮
        if msg_id.is_empty() {
            self.highlight_message_id.set(None);
        } else {
            self.highlight_message_id.set(Some(msg_id));
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_result.set(None);
        self.preview_conversation_id.set(None);
        self.highlight_message_id.set(None);
    }

    pub fn set_recent_page(&mut self, page: usize) {
        if page >= 1 {
            self.recent_page.set(page);
        }
    }

    pub fn recent_total_pages(&self) -> usize {
        let page_size = *self.recent_page_size.read();
        let total = *self.recent_total.read();
        if page_size == 0 {
            return 1;
        }
        (total + page_size - 1) / page_size
    }

    pub async fn load_recent_conversations(&mut self) {
        let page = *self.recent_page.read();
        let page_size = *self.recent_page_size.read();
        if let Some(store) = crate::services::conversation_store::get_store() {
            match store.load_conversation_list_paged(page, page_size).await {
                Ok((items, total)) => {
                    self.recent_items.set(items);
                    self.recent_total.set(total);
                }
                Err(_) => {
                    self.recent_items.set(Vec::new());
                    self.recent_total.set(0);
                }
            }
        }
    }

    /// 执行全文搜索，失败时打印错误并返回空列表。
    async fn run_fulltext_search(query: &str) -> Vec<SearchResult> {
        match crate::services::search::fulltext_search(query, SEARCH_PAGE_SIZE).await {
            Ok(results) => results,
            Err(e) => {
                eprintln!("[xechat] Fulltext search failed: {}", e);
                Vec::new()
            }
        }
    }

    /// 执行语义搜索，失败时返回空列表。
    async fn run_semantic_search(query: &str) -> Vec<SearchResult> {
        crate::services::search::semantic_search(query, SEARCH_PAGE_SIZE)
            .await
            .unwrap_or_default()
    }

    /// 根据搜索类型执行搜索并返回结果。
    pub async fn dispatch_search(query: &str, search_type: &SearchType) -> Vec<SearchResult> {
        match search_type {
            SearchType::FullText => Self::run_fulltext_search(query).await,
            SearchType::Semantic => Self::run_semantic_search(query).await,
            SearchType::Hybrid => {
                let fulltext = Self::run_fulltext_search(query).await;
                let semantic = Self::run_semantic_search(query).await;
                if semantic.is_empty() {
                    fulltext
                } else {
                    crate::services::search::hybrid::reciprocal_rank_fusion(
                        fulltext,
                        semantic,
                        60,
                    )
                }
            }
        }
    }

    pub async fn execute_search(&mut self) {
        let query = self.query.read().clone();
        if query.trim().is_empty() {
            self.results.set(Vec::new());
            return;
        }

        self.is_searching.set(true);

        let search_type = self.search_type.read().clone();
        let results = Self::dispatch_search(&query, &search_type).await;

        self.results.set(results);
        self.is_searching.set(false);
    }
}
