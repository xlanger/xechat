//! 应用配置数据模型。
//!
//! 本模块定义了 XEChat 的配置数据结构，包括应用级配置、模型提供商配置
//! 和单个模型的参数配置。所有结构体均支持 TOML 序列化/反序列化，
//! 与项目根目录的 `config.toml` 文件一一对应。
//!
//! # 数据结构层级
//!
//! ```text
//! XEChatConfig                       ← 应用顶层配置
//!   ├── model_providers: map         ← 提供商字典，key 为提供商标识符
//!   │     └── ModelProvider          ← 单个提供商配置
//!   │           └── models: map      ← 模型字典，key 为模型名
//!   │                 └── ModelConfig ← 单个模型参数
//!   ├── model                        ← 默认模型
//!   ├── model_provider               ← 默认提供商
//!   ├── memory: MemoryConfig         ← 记忆管线配置
//!   └── preferences: PreferencesConfig ← 用户偏好配置
//!         └── ollama: OllamaPreferences ← Ollama 偏好
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 解析字符串中的 `${VAR}` 和 `$VAR` 环境变量引用
pub fn resolve_env_vars_in_str(input: &str) -> String {
    use regex::Regex;
    let re_braced = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}").unwrap();
    let re_simple = Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").unwrap();

    let mut result = input.to_string();
    result = re_braced
        .replace_all(&result, |caps: &regex::Captures| {
            let var_name = caps.get(1).unwrap().as_str();
            std::env::var(var_name)
                .unwrap_or_else(|_| caps.get(0).unwrap().as_str().to_string())
        })
        .to_string();
    result = re_simple
        .replace_all(&result, |caps: &regex::Captures| {
            let var_name = caps.get(1).unwrap().as_str();
            std::env::var(var_name)
                .unwrap_or_else(|_| caps.get(0).unwrap().as_str().to_string())
        })
        .to_string();
    result
}

/// 应用顶层配置，对应 `config.toml` 文件的完整结构。
///
/// 包含默认模型设置、上下文管理参数和多提供商配置。
/// 通过 [`Default`] 实现提供内置的 DeepSeek 默认配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct XEChatConfig {
    /// 默认使用的模型标识符（如 `"deepseek-v4-flash"`）
    pub model: String,
    /// 默认使用的提供商标识符，对应 [`model_providers`](XEChatConfig::model_providers) 中的 key
    pub model_provider: String,
    /// 界面主题模式（`"system"`、`"dark"`、`"light"`）
    pub theme: String,
    /// 界面语言（`"system"`、`"zh"`、`"en"`）
    pub language: String,
    /// 用户时区偏好（IANA 时区标识符，如 `"Asia/Shanghai"`、`"America/New_York"`），
    /// `"system"` 表示使用系统本地时区
    pub timezone: String,
    /// 最大上下文 token 数量，`None` 表示不限制。
    /// 用于控制发送给模型的上下文窗口大小
    pub max_context_tokens: Option<u32>,
    /// 是否启用自动上下文管理（自动截断/压缩超长对话历史），`None` 表示使用默认值
    pub auto_context_management: Option<bool>,
    /// 所有可用模型提供商的配置映射表，key 为提供商标识符
    pub model_providers: HashMap<String, ModelProvider>,
    /// 记忆管线配置
    pub memory: MemoryConfig,
    /// 用户偏好配置（模型提供商选择、Ollama 偏好等）
    pub preferences: PreferencesConfig,
}

/// 单个模型提供商的完整配置。
///
/// 包含 API 接入信息、超时设置和该提供商下所有模型的参数配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProvider {
    /// 提供商的显示名称（如 `"DeepSeek"`）
    pub name: String,
    /// API 密钥，用于 HTTP Bearer token 认证
    pub api_key: String,
    /// API 基础 URL（不含路径后缀），如 `"https://api.deepseek.com"`
    pub base_url: String,
    /// HTTP 请求超时时间（秒），`None` 表示使用默认值
    pub timeout: Option<u64>,
    /// 该提供商下所有可用模型的参数配置映射表，key 为模型名
    pub models: HashMap<String, ModelConfig>,
}

/// 单个模型的采样参数配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 模型最大输出 token 数
    pub max_tokens: u32,
    /// 采样温度 (0.0–2.0)，值越高输出越随机
    pub temperature: f32,
    /// 核采样阈值 (0.0–1.0)，与 temperature 二选一使用
    pub top_p: f32,
    /// 频率惩罚 (0.0–2.0)，降低模型重复已出现词汇的倾向
    #[serde(default)]
    pub frequency_penalty: f32,
    /// 存在惩罚 (0.0–2.0)，降低模型谈论新话题的倾向
    #[serde(default)]
    pub presence_penalty: f32,
    /// 上下文窗口大小（输入+输出总 token 数上限）
    #[serde(default = "default_context_window")]
    pub context_window: u32,
    /// 自定义停止序列，模型遇到这些字符串时停止生成
    #[serde(default)]
    pub stop_sequences: Vec<String>,
}

fn default_context_window() -> u32 {
    8192
}

impl ModelProvider {
    /// 解析 API Key，按优先级回退：
    /// 1. 配置文件中非空值（解析 `${VAR}` 引用）
    /// 2. 环境变量 `{PROVIDER_KEY_UPPER}_API_KEY`
    /// 3. 无可用值时返回 `None`
    pub fn resolve_api_key(&self, provider_key: &str) -> Option<String> {
        if !self.api_key.is_empty() {
            let resolved = resolve_env_vars_in_str(&self.api_key);
            if !resolved.is_empty() {
                return Some(resolved);
            }
        }
        let env_var = format!("{}_API_KEY", provider_key.to_uppercase().replace('-', "_"));
        std::env::var(env_var).ok().filter(|s| !s.is_empty())
    }

    /// 解析 Base URL，逻辑同 `resolve_api_key`
    pub fn resolve_base_url(&self, provider_key: &str) -> Option<String> {
        if !self.base_url.is_empty() {
            let resolved = resolve_env_vars_in_str(&self.base_url);
            if !resolved.is_empty() {
                return Some(resolved);
            }
        }
        let env_var = format!("{}_BASE_URL", provider_key.to_uppercase().replace('-', "_"));
        std::env::var(env_var).ok().filter(|s| !s.is_empty())
    }
}



/// 记忆管线配置。
///
/// 仅包含与记忆检索相关的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// 记忆检索返回的最大结果数
    #[serde(default = "default_max_memory_results")]
    pub max_memory_results: u32,
}

fn default_max_memory_results() -> u32 {
    5
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memory_results: 5,
        }
    }
}

/// Ollama 用户偏好配置。
///
/// 包含 Ollama 服务地址和用户指定的嵌入模型名称。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OllamaPreferences {
    /// Ollama 服务地址（如 `http://localhost:11434`）
    #[serde(default = "default_ollama_host")]
    pub host: String,
    /// 用户指定的 Ollama 嵌入模型名称
    #[serde(default)]
    pub embed_model: String,
}

fn default_ollama_host() -> String {
    "http://localhost:11434".to_string()
}

impl Default for OllamaPreferences {
    fn default() -> Self {
        Self {
            host: default_ollama_host(),
            embed_model: String::new(),
        }
    }
}

/// 用户偏好配置。
///
/// 包含嵌入模型提供商选择和各提供商的用户偏好设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreferencesConfig {
    /// 嵌入模型提供商：`"default"`（内置 Qwen3-Embedding-0.6B）或 `"ollama"`
    #[serde(default)]
    pub embed_provider: String,
    /// Ollama 偏好配置
    #[serde(default)]
    pub ollama: OllamaPreferences,
}

impl Default for PreferencesConfig {
    fn default() -> Self {
        Self {
            embed_provider: "default".to_string(),
            ollama: OllamaPreferences::default(),
        }
    }
}

impl Default for XEChatConfig {
    fn default() -> Self {
        let mut model_providers = HashMap::new();

        let mut deepseek = ModelProvider {
            name: "DeepSeek".to_string(),
            api_key: "".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            timeout: Some(120),
            models: HashMap::new(),
        };

        let mut models = HashMap::new();
        models.insert(
            "deepseek-v4-flash".to_string(),
            ModelConfig {
                max_tokens: 384000,
                temperature: 0.2,
                top_p: 0.95,
                frequency_penalty: 0.0,
                presence_penalty: 0.0,
                context_window: 131072,
                stop_sequences: vec![],
            },
        );
        models.insert(
            "deepseek-v4-pro".to_string(),
            ModelConfig {
                max_tokens: 384000,
                temperature: 0.1,
                top_p: 0.9,
                frequency_penalty: 0.0,
                presence_penalty: 0.0,
                context_window: 131072,
                stop_sequences: vec![],
            },
        );

        deepseek.models = models;
        model_providers.insert("deepseek".to_string(), deepseek);

        let ollama = ModelProvider {
            name: "Ollama".to_string(),
            api_key: "".to_string(),
            base_url: "http://localhost:11434".to_string(),
            timeout: Some(120),
            models: HashMap::new(),
        };
        model_providers.insert("ollama".to_string(), ollama);

        let openai_compatible = ModelProvider {
            name: "OpenAI Compatible".to_string(),
            api_key: "".to_string(),
            base_url: "".to_string(),
            timeout: Some(120),
            models: HashMap::new(),
        };
        model_providers.insert("openai-compatible".to_string(), openai_compatible);

        let openai = ModelProvider {
            name: "OpenAI".to_string(),
            api_key: "".to_string(),
            base_url: "https://api.openai.com".to_string(),
            timeout: Some(120),
            models: HashMap::new(),
        };
        model_providers.insert("openai".to_string(), openai);

        Self {
            model: "deepseek-v4-flash".to_string(),
            model_provider: "deepseek".to_string(),
            theme: "system".to_string(),
            language: "system".to_string(),
            timezone: "system".to_string(),
            max_context_tokens: Some(8192),
            auto_context_management: Some(true),
            model_providers,
            memory: MemoryConfig::default(),
            preferences: PreferencesConfig::default(),
        }
    }
}
