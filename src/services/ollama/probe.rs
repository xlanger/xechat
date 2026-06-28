//! Ollama 服务自动探测。
//!
//! 探测 Ollama 服务是否运行、获取版本号、枚举已安装模型、
//! 自动分类嵌入模型和聊天模型。

use super::OllamaStatus;
use crate::services::ollama::OllamaConfig;

/// Ollama 探测请求超时时间（秒）。
const PROBE_TIMEOUT_SECS: u64 = 3;

/// 构建带超时的 HTTP 客户端，供所有探测函数共用。
fn build_probe_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(PROBE_TIMEOUT_SECS))
        .build()
        .expect("reqwest client build should not fail")
}

/// 根据模型名称启发式分类为 "embed" 或 "chat"。
///
/// 仅识别 Qwen3-Embedding 系列模型为嵌入模型：
/// - 名称同时包含 "qwen3" 和 "embed" → "embed"
/// - 其他 → "chat"
///
/// 注意：`filter_models_by_category` 优先使用 `/api/tags` 返回的
/// `details.family` 字段识别，此函数仅作为 fallback。
pub fn classify_model(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.contains("qwen3") && lower.contains("embed") {
        "embed"
    } else {
        "chat"
    }
}

/// 检测 Ollama 版本端点，返回 (available, version)。
pub async fn check_version(client: &reqwest::Client, host: &str) -> Option<(bool, String)> {
    let resp = client
        .get(format!("{}/api/version", host))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let version = resp
        .json::<serde_json::Value>()
        .await
        .map(|json| json["version"].as_str().unwrap_or("unknown").to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    Some((true, version))
}

/// 从 /api/tags 响应中提取模型并按分类填充 status。
///
/// 优先使用 `details.family` 识别嵌入模型，fallback 到名称匹配。
pub fn populate_models_from_json(status: &mut OllamaStatus, json: serde_json::Value) {
    let Some(models) = json["models"].as_array() else {
        return;
    };
    for m in models {
        let name = m["name"].as_str().unwrap_or("");
        let category = classify_by_details(m).unwrap_or_else(|| classify_model(name));
        match category {
            "embed" if status.embed_model.is_none() => {
                status.embed_model = Some(name.to_string());
            }
            "chat" if status.chat_model.is_none() => {
                status.chat_model = Some(name.to_string());
            }
            _ => {}
        }
    }
}

/// 获取模型列表并填充到 status。
async fn fetch_and_populate_models(client: &reqwest::Client, host: &str, status: &mut OllamaStatus) {
    let resp = client
        .get(format!("{}/api/tags", host))
        .send()
        .await;

    if let Ok(r) = resp
        && let Ok(json) = r.json::<serde_json::Value>().await {
            populate_models_from_json(status, json);
        }
}

/// 应用用户偏好覆盖自动探测的模型选择。
pub fn apply_preferred_models(config: &OllamaConfig, status: &mut OllamaStatus) {
    if let Some(ref preferred) = config.preferred_embed {
        status.embed_model = Some(preferred.clone());
    }
    if let Some(ref preferred) = config.preferred_chat {
        status.chat_model = Some(preferred.clone());
    }
}

/// 探测 Ollama 服务并返回运行时状态。
///
/// 依次执行：
/// 1. GET /api/version — 检查服务是否运行
/// 2. GET /api/tags — 枚举已安装模型
/// 3. 自动分类模型（嵌入/聊天）
/// 4. 应用用户偏好覆盖
pub async fn probe(config: &OllamaConfig) -> OllamaStatus {
    let mut status = OllamaStatus {
        host: config.host.clone(),
        ..Default::default()
    };

    let client = build_probe_client();

    let Some((available, version)) = check_version(&client, &config.host).await else {
        return status;
    };
    status.available = available;
    status.version = version;

    fetch_and_populate_models(&client, &config.host, &mut status).await;
    apply_preferred_models(config, &mut status);

    status
}

/// 探测 Ollama 服务是否可达。
pub async fn probe_host(host: &str) -> bool {
    let client = build_probe_client();

    client
        .get(format!("{}/api/version", host))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// 从 /api/tags 响应 JSON 中提取指定分类的模型名称列表。
///
/// # 嵌入模型识别策略
///
/// 优先使用 `/api/tags` 返回的 `details.family` 字段：
/// - `family` 包含 "qwen3-embedding" → 嵌入模型
///
/// 若 `details.family` 不可用（旧版 Ollama），fallback 到 [`classify_model`] 名称匹配。
///
/// # Arguments
///
/// * `json` - Ollama /api/tags 响应的 JSON
/// * `category` - 目标分类，"embed" 或 "chat"
pub fn filter_models_by_category(json: &serde_json::Value, category: &str) -> Vec<String> {
    let Some(models) = json["models"].as_array() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|m| {
            let name = m["name"].as_str()?;
            let model_category = classify_by_details(m).unwrap_or_else(|| classify_model(name));
            if model_category == category {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// 从模型 JSON 对象的 `details.family` 字段识别模型分类。
///
/// - `family` 含 "qwen3-embedding" → "embed"
/// - 其他 → `None`（无法从 family 确定，fallback 到名称匹配）
///
/// 返回 `None` 而非 `"chat"`，确保名称匹配 fallback 仍能生效。
/// 例如 `qwen3-embedding:0.6b` 的 family 可能只是 `"qwen3-embedding"`，
/// 但如果 family 值为 `"qwen3"` 则需 fallback 到名称匹配。
fn classify_by_details(model_json: &serde_json::Value) -> Option<&'static str> {
    let family = model_json["details"]["family"].as_str()?.to_lowercase();
    if family.contains("qwen3-embedding") || family.contains("qwen3_embedding") {
        Some("embed")
    } else {
        None
    }
}

/// 获取 Ollama 服务中已安装的嵌入模型名称列表。
///
/// # Arguments
///
/// * `host` - Ollama 服务地址（如 `http://localhost:11434`）
///
/// # Returns
///
/// 分类为 "embed" 的模型名称列表。若服务不可达或请求失败，返回空列表。
pub async fn fetch_embed_models(host: &str) -> Vec<String> {
    let client = build_probe_client();

    let resp = client
        .get(format!("{}/api/tags", host))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            if let Ok(json) = r.json::<serde_json::Value>().await {
                return filter_models_by_category(&json, "embed");
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// 获取 Ollama 服务中已安装的聊天模型名称列表。
///
/// # Arguments
///
/// * `host` - Ollama 服务地址（如 `http://localhost:11434`）
///
/// # Returns
///
/// 分类为 "chat" 的模型名称列表。若服务不可达或请求失败，返回空列表。
pub async fn fetch_chat_models(host: &str) -> Vec<String> {
    let client = build_probe_client();

    let resp = client
        .get(format!("{}/api/tags", host))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            if let Ok(json) = r.json::<serde_json::Value>().await {
                return filter_models_by_category(&json, "chat");
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// 检查模型名称是否匹配指定模型（含标签前缀匹配）。
///
/// `nomic-embed-text` 匹配 `nomic-embed-text:latest`。
#[inline]
pub fn model_name_matches(name: &str, model: &str) -> bool {
    name == model || name.starts_with(&format!("{}:", model))
}

/// 在模型列表中查找指定模型是否存在。
pub fn find_model_in_json(json: &serde_json::Value, model: &str) -> bool {
    let Some(models) = json["models"].as_array() else {
        return false;
    };
    models.iter().any(|m| {
        m["name"].as_str()
            .map(|n| model_name_matches(n, model))
            .unwrap_or(false)
    })
}

/// 探测 Ollama 服务中指定模型是否存在。
pub async fn probe_model(host: &str, model: &str) -> bool {
    let client = build_probe_client();

    let resp = client
        .get(format!("{}/api/tags", host))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            if let Ok(json) = r.json::<serde_json::Value>().await {
                return find_model_in_json(&json, model);
            }
            false
        }
        _ => false,
    }
}
