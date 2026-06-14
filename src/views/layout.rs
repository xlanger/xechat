use dioxus::prelude::*;
use dioxus_style::with_css;
use rust_i18n::t;
use crate::hooks::{use_app, use_conversation, use_ui};
use crate::components::sidebar::Sidebar;
use crate::views::conversation::ConversationView;
use crate::views::SettingsView;
use crate::views::welcome::Welcome;
use crate::state::MainRoute;
use crate::stores::ui::ToastKind;

/// 全局快捷键 JS：注册 document 级别 keydown 监听，
/// 检测到 Command+K（macOS）或 Ctrl+K（Windows/Linux）时点击隐藏按钮。
/// macOS WebKit 会拦截 Command+K 的 DOM onkeydown 事件，必须用 JS 全局监听绕过。
static GLOBAL_SHORTCUT_JS: &str = r#"
if(!window._xechatShortcutInit){
window._xechatShortcutInit=1;
document.addEventListener('keydown',function(e){
if((e.metaKey||e.ctrlKey)&&e.key==='k'){
e.preventDefault();
var btn=document.getElementById('__xechat_new_chat_btn');
if(btn)btn.click();
}
});
}
"#;

#[with_css(css, "styles/views/layout.scss")]
/// 应用布局组件，组装侧边栏与主内容区。
///
/// 主内容区根据 MainRoute 路由状态切换三种视图：
/// - Welcome: 欢迎页（无对话时默认显示）
/// - Conversation(id): 对话界面
/// - Settings: 设置页面
#[component]
pub fn Layout() -> Element {
    let app_store = use_app();
    let mut conv_store = use_conversation();
    let ui_store = use_ui();

    let route = app_store.main_route.read().clone();

    // 处理待发送消息：ChatInput 通过 pending_send 信号传递发送请求，
    // Layout 永不 unmount，spawn 的异步任务不会被取消
    let pending = conv_store.pending_send.read().clone();
    if let Some((content, config)) = pending {
        conv_store.pending_send.set(None);
        let mut conv_store = conv_store.clone();
        let mut active_toast = ui_store.active_toast;
        let toast_callback = move |kind: ToastKind, msg: String| {
            let toast_msg = format!("{} ({})", t!("error.api.http-error"), msg);
            active_toast.set(Some(crate::stores::ui::Toast {
                message: toast_msg,
                kind,
                duration_ms: 6000,
            }));
        };
        spawn(async move {
            conv_store.send_message(content, config, toast_callback).await;
        });
    }

    // 注入全局快捷键 JS
    use_effect(|| {
        let _ = dioxus::desktop::window().webview.evaluate_script(GLOBAL_SHORTCUT_JS);
    });

    // 启动时异步检测网络连通性
    use_future(move || {
        let mut store = app_store;
        async move {
            store.check_network().await;
        }
    });

    // 启动心跳监测服务（仅首次挂载时执行）
    use_effect(move || {
        let network_sig = app_store.network_available;
        let embedder_ready_sig = conv_store.embedder_ready;
        let toast_sig = ui_store.active_toast;

        crate::services::heartbeat::init(
            network_sig,
            embedder_ready_sig,
            toast_sig,
        );
    });

    // Command+K / Ctrl+K 新建对话
    let new_chat = {
        let mut app_store = app_store.clone();
        let mut conv_store = conv_store.clone();
        move |_| {
            conv_store.current_conversation_id.set(None);
            app_store.navigate_to(MainRoute::Welcome);
        }
    };

    rsx! {
        div {
            class: "{css::app_root}",
            // 隐藏按钮，供全局快捷键 JS 触发
            button {
                id: "__xechat_new_chat_btn",
                style: "display:none",
                onclick: new_chat,
            }
            div {
                class: "{css::titlebar}",
                onmousedown: |_| {
                    dioxus::desktop::window().drag();
                },
                ondoubleclick: |_| {
                    dioxus::desktop::window().toggle_maximized();
                },
            }
            div {
                class: "{css::app_body}",
                Sidebar {}
                div {
                    class: "{css::main_content}",
                    match route {
                        MainRoute::Settings => rsx! { SettingsView {} },
                        MainRoute::Search => rsx! { crate::views::search::SearchView {} },
                        MainRoute::Conversation(_) => rsx! { ConversationView {} },
                        MainRoute::Welcome => rsx! { Welcome {} },
                    }
                }
            }
            // 重建进度遮罩
            if *conv_store.rebuild_in_progress.read() {
                {
                    let (current, total) = *conv_store.rebuild_progress.read();
                    let msg = t!("rebuild.progress", current = current, total = total).to_string();
                    rsx! {
                        div {
                            class: "{css::rebuild_overlay}",
                            div {
                                class: "{css::rebuild_dialog}",
                                div {
                                    class: "{css::rebuild_spinner}",
                                }
                                p {
                                    class: "{css::rebuild_text}",
                                    "{msg}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
