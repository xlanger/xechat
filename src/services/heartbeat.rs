//! 状态心跳监测服务。
//!
//! 启动三个独立的 tokio interval 任务，周期性检测：
//! - 网络连通性（30s）
//! - 嵌入模型就绪（60s）
//! - Ollama 服务连通性（30s，仅 Ollama 模式）
//!
//! 状态变化时通过 toast 通知，并更新对应的 Signal 驱动侧边栏图标刷新。
//!
//! # 架构
//!
//! 由于 Dioxus 0.7 的 `Signal<T>` 使用 `UnsyncStorage`，不能跨线程共享，
//! 因此采用 channel 模式：
//! - 心跳检测逻辑在 `tokio::spawn` 中执行（IO/网络操作）
//! - 状态变更通过 `mpsc::channel` 发送事件
//! - Signal 更新在 `dioxus::prelude::spawn` 中执行（Dioxus 异步上下文）

use std::sync::atomic::{AtomicBool, Ordering};

use dioxus::prelude::*;
use tokio::sync::mpsc;

/// 心跳运行标志，用于优雅停止所有任务。
static RUNNING: AtomicBool = AtomicBool::new(false);

/// 心跳事件类型。
enum HeartbeatEvent {
    /// 网络状态变化 (is_online)
    Network(bool),
    /// 嵌入模型就绪状态变化 (is_ready)
    EmbedderReady(bool),
    /// Ollama 连通状态变化 (暂不暴露到 UI)
    OllamaOnline,
    /// Toast 通知 (locale key)
    Toast(&'static str),
}

/// 初始化心跳服务并启动所有心跳任务。
///
/// 必须在应用启动的 Dioxus 组件上下文中调用（如 Layout），
/// 以便克隆 Signal 并使用 `dioxus::prelude::spawn` 监听 channel。
/// 多次调用安全（内部检查是否已启动）。
///
/// # Arguments
///
/// * `network` - 网络可用性 Signal（来自 AppStore）
/// * `embedder_ready` - 嵌入模型就绪 Signal（来自 ConversationStore）
/// * `toast` - Toast 通知 Signal（来自 UIStore.active_toast）
pub fn init(
    network: Signal<bool>,
    embedder_ready: Signal<bool>,
    toast: Signal<Option<crate::stores::ui::Toast>>,
) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let (tx, rx) = mpsc::channel::<HeartbeatEvent>(32);

    spawn_heartbeat_tasks(tx);
    spawn_heartbeat_loop(rx, network, embedder_ready, toast);

    eprintln!("[xechat] Heartbeat service started");
}

/// 启动三个独立的心跳检测任务（网络、嵌入器、Ollama）。
fn spawn_heartbeat_tasks(tx: mpsc::Sender<HeartbeatEvent>) {
    let net_tx = tx.clone();
    tokio::spawn(async move { network_heartbeat_task(net_tx).await });

    let emb_tx = tx.clone();
    tokio::spawn(async move { embedder_heartbeat_task(emb_tx).await });

    tokio::spawn(async move { ollama_heartbeat_task(tx).await });
}

/// 在 Dioxus 异步上下文中监听心跳事件并更新对应 Signal。
fn spawn_heartbeat_loop(
    mut rx: mpsc::Receiver<HeartbeatEvent>,
    network: Signal<bool>,
    embedder_ready: Signal<bool>,
    toast: Signal<Option<crate::stores::ui::Toast>>,
) {
    spawn(async move {
        while let Some(event) = rx.recv().await {
            handle_heartbeat_event(event, network, embedder_ready, toast);
        }
    });
}

/// 处理单个心跳事件，更新对应的 Signal。
fn handle_heartbeat_event(
    event: HeartbeatEvent,
    mut network: Signal<bool>,
    mut embedder_ready: Signal<bool>,
    mut toast: Signal<Option<crate::stores::ui::Toast>>,
) {
    match event {
        HeartbeatEvent::Network(online) => {
            network.set(online);
        }
        HeartbeatEvent::EmbedderReady(ready) => {
            embedder_ready.set(ready);
        }
        HeartbeatEvent::OllamaOnline => {
            // Ollama online 暂不暴露到 UI
        }
        HeartbeatEvent::Toast(key) => {
            use rust_i18n::t;
            let msg = t!(key).to_string();
            toast.set(Some(crate::stores::ui::Toast {
                message: msg,
                kind: crate::stores::ui::ToastKind::Info,
                duration_ms: 4000,
            }));
        }
    }
}

/// 停止所有心跳任务。
pub fn stop() {
    RUNNING.store(false, Ordering::SeqCst);
}

// ── Network Heartbeat ────────────────────────────────────────────

async fn network_heartbeat_task(tx: mpsc::Sender<HeartbeatEvent>) {
    // 首次延迟 15s，避免与启动时的 check_network 冲突
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    let mut last_state: Option<bool> = None;

    while RUNNING.load(Ordering::SeqCst) {
        interval.tick().await;

        let online = check_network_online().await;

        if last_state != Some(online) {
            eprintln!(
                "[xechat:heartbeat] network {:?} -> {}",
                last_state, online
            );
            let _ = tx.send(HeartbeatEvent::Network(online)).await;
            let _ = tx
                .send(HeartbeatEvent::Toast(if online {
                    "status.network-restored"
                } else {
                    "status.network-lost"
                }))
                .await;
            last_state = Some(online);
        }
    }
}

/// 轻量网络检测：复用 check_network 逻辑但独立于 AppStore。
async fn check_network_online() -> bool {
    let config = crate::services::config::load_config().unwrap_or_default();
    let primary_url =
        crate::stores::app::AppStore::resolve_primary_url(&config);

    // 本地服务视为在线
    if primary_url
        .as_deref()
        .map_or(false, crate::stores::app::AppStore::is_local_url)
    {
        return true;
    }

    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .connect_timeout(std::time::Duration::from_secs(3))
        .build()
    else {
        return false;
    };

    // 主检测：对话模型 host
    if let Some(ref url) = primary_url {
        let target = url.trim_end_matches('/');
        if client.head(target).send().await.is_ok() {
            return true;
        }
    }

    // 辅检测：公共端点
    client.head("https://github.com").send().await.is_ok()
}

// ── Embedder Heartbeat ────────────────────────────────────────

async fn embedder_heartbeat_task(tx: mpsc::Sender<HeartbeatEvent>) {
    // 首次延迟 20s，避免与启动时的 init_embedder 冲突
    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut last_state: Option<bool> = None;

    while RUNNING.load(Ordering::SeqCst) {
        interval.tick().await;

        let ready = check_embedder_ready().await;

        if last_state != Some(ready) {
            eprintln!(
                "[xechat:heartbeat] embedder {:?} -> {}",
                last_state, ready
            );
            let _ = tx.send(HeartbeatEvent::EmbedderReady(ready)).await;
            let _ = tx
                .send(HeartbeatEvent::Toast(if ready {
                    "status.embedder-ready"
                } else {
                    "status.embedder-unavailable"
                }))
                .await;
            last_state = Some(ready);
        }
    }
}

/// 检测嵌入模型是否可用。
async fn check_embedder_ready() -> bool {
    let config = crate::services::config::load_config().unwrap_or_default();

    if crate::stores::conversation::should_enable_ollama(&config) {
        // Ollama 模式：探测 /api/version
        let host = crate::stores::conversation::ConversationStore::resolve_ollama_host(
            &config.preferences.ollama.host,
        );
        crate::services::ollama::probe::probe_host(&host).await
    } else if crate::stores::conversation::is_ollama_provider_selected(&config) {
        // [加固] ollama provider 已选但 model 尚未配置 → 不就绪
        false
    } else {
        // 内置模式：检查模型文件是否存在
        crate::services::model_downloader::is_model_ready()
    }
}

// ── Ollama Service Heartbeat ──────────────────────────────────

async fn ollama_heartbeat_task(tx: mpsc::Sender<HeartbeatEvent>) {
    // 首次延迟 25s
    tokio::time::sleep(std::time::Duration::from_secs(25)).await;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
    let mut last_state: Option<bool> = None;

    while RUNNING.load(Ordering::SeqCst) {
        interval.tick().await;

        let config = crate::services::config::load_config().unwrap_or_default();

        // 仅当配置了 Ollama 且 host 非空时检测
        if config.preferences.ollama.host.is_empty() {
            tokio::time::sleep(interval.period()).await;
            continue;
        }

        let host = crate::stores::conversation::ConversationStore::resolve_ollama_host(
            &config.preferences.ollama.host,
        );
        let online = crate::services::ollama::probe::probe_host(&host).await;

        if last_state != Some(online) {
            eprintln!(
                "[xechat:heartbeat] ollama {:?} -> {}",
                last_state, online
            );
            let _ = tx.send(HeartbeatEvent::OllamaOnline).await;
            let _ = tx
                .send(HeartbeatEvent::Toast(if online {
                    "status.ollama-connected"
                } else {
                    "status.ollama-disconnected"
                }))
                .await;
            last_state = Some(online);
        }
    }
}
