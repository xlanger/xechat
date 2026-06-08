//! 服务层入口模块。
//!
//! 本层是 XEChat 架构中的 services 层，负责业务逻辑编排与外部服务交互：
//! - `config`：配置文件的加载、保存与环境变量解析
//! - `conversation`：对话数据的持久化存储、迁移与 CRUD 操作
//! - `paths`：应用各数据文件的路径管理
//! - `ai`：统一的 AI 调用抽象层，支持多 Provider 动态路由
//! - `embedder`：文本向量化（Embedding）抽象层，支持 E5 GGUF 与 Ollama 两种后端
//! - `intent`：用户意图识别抽象层，判断是否需要检索记忆
//! - `vector_store`：向量存储抽象层，支持 LanceDB 后端

pub mod conversation;
pub mod config;
pub mod paths;
pub mod ai;
pub mod embedder;
pub mod intent;
pub mod vector_store;
pub mod memory;
pub mod ollama;
pub mod search;
pub mod conversation_store;
