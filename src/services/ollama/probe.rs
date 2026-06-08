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
pub fn classify_model(name: &str) -> &'static str {
    let lower = name.to_lowercase();
    if lower.contains("embed")
        || lower.contains("jina")
        || lower.contains("nomic")
        || lower.contains("gte")
        || lower.contains("bge-")
        || lower.contains("e5-")
    {
        "embed"
    } else {
        "chat"
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

    let version_resp = client
        .get(format!("{}/api/version", config.host))
        .send()
        .await;

    match version_resp {
        Ok(resp) if resp.status().is_success() => {
            status.available = true;
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                status.version = json["version"].as_str().unwrap_or("unknown").to_string();
            }
        }
        _ => return status,
    }

    let models_resp = client
        .get(format!("{}/api/tags", config.host))
        .send()
        .await;

    if let Ok(resp) = models_resp {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            if let Some(models) = json["models"].as_array() {
                for m in models {
                    let name = m["name"].as_str().unwrap_or("");
                    match classify_model(name) {
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
        }
    }

    if let Some(ref preferred) = config.preferred_embed {
        status.embed_model = Some(preferred.clone());
    }
    if let Some(ref preferred) = config.preferred_chat {
        status.chat_model = Some(preferred.clone());
    }

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
                if let Some(models) = json["models"].as_array() {
                    return models
                        .iter()
                        .filter_map(|m| {
                            let name = m["name"].as_str()?;
                            if classify_model(name) == "embed" {
                                Some(name.to_string())
                            } else {
                                None
                            }
                        })
                        .collect();
                }
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
                if let Some(models) = json["models"].as_array() {
                    return models
                        .iter()
                        .filter_map(|m| {
                            let name = m["name"].as_str()?;
                            if classify_model(name) == "chat" {
                                Some(name.to_string())
                            } else {
                                None
                            }
                        })
                        .collect();
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
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
                if let Some(models) = json["models"].as_array() {
                    return models.iter().any(|m| {
                        m["name"].as_str()
                            .map(|n| n == model || n.starts_with(&format!("{}:", model)))
                            .unwrap_or(false)
                    });
                }
            }
            false
        }
        _ => false,
    }
}
