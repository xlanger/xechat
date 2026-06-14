//! 通用提示浮窗组件。
//!
//! 四象限定位：气泡偏移到触发元素的左上/右上/左下/右下，
//! 箭头在气泡靠近触发元素那一侧的边缘角落，指向触发元素中心。
//! 自动选择最优象限（空间最大的方向）。

use dioxus::prelude::*;
use dioxus_style::with_css;

/// 气泡与触发元素的间距。
const BUBBLE_GAP: f64 = 8.0;
/// 视口边缘安全距离。
const VIEWPORT_PADDING: f64 = 8.0;

/// JS 脚本：延迟计算最优象限并定位气泡。
/// setTimeout 等 VDOM 更新 data-visible 后再执行。
fn position_script() -> String {
    format!(
        r#"
        setTimeout(function() {{
            var wrappers = document.querySelectorAll('[data-tooltip="wrapper"][data-visible="true"]');
            wrappers.forEach(function(wrapper) {{
                var bubble = wrapper.querySelector('[data-tooltip="bubble"]');
                if (!bubble) return;
                positionBubble(wrapper, bubble);
            }});

            function positionBubble(wrapper, bubble) {{
                var rect = wrapper.getBoundingClientRect();
                var cx = rect.left + rect.width / 2;
                var cy = rect.top + rect.height / 2;

                // 临时设为 fixed 获取真实尺寸
                bubble.style.position = 'fixed';
                bubble.style.visibility = 'hidden';
                bubble.style.left = '0';
                bubble.style.top = '0';
                bubble.style.transform = 'none';
                bubble.style.bottom = '';
                bubble.style.right = '';
                bubble.style.marginLeft = '';
                var bRect = bubble.getBoundingClientRect();
                var bw = bRect.width;
                var bh = bRect.height;

                // 计算四个象限的可用空间
                var spaceTop = rect.top;
                var spaceBottom = window.innerHeight - rect.bottom;
                var spaceLeft = rect.left;
                var spaceRight = window.innerWidth - rect.right;

                // 选择垂直方向：上方空间更大则上，否则下
                var showAbove = spaceTop >= spaceBottom;
                // 选择水平方向：右侧空间更大则右，否则左
                var showRight = spaceRight >= spaceLeft;

                var top, left;

                if (showAbove && showRight) {{
                    // 右上：气泡在触发元素右上方
                    top = rect.top - bh - {gap};
                    left = rect.left;
                    bubble.setAttribute('data-pos', 'top-right');
                }} else if (showAbove && !showRight) {{
                    // 左上：气泡在触发元素左上方
                    top = rect.top - bh - {gap};
                    left = rect.left + rect.width - bw;
                    bubble.setAttribute('data-pos', 'top-left');
                }} else if (!showAbove && showRight) {{
                    // 右下：气泡在触发元素右下方
                    top = rect.bottom + {gap};
                    left = rect.left;
                    bubble.setAttribute('data-pos', 'bottom-right');
                }} else {{
                    // 左下：气泡在触发元素左下方
                    top = rect.bottom + {gap};
                    left = rect.left + rect.width - bw;
                    bubble.setAttribute('data-pos', 'bottom-left');
                }}

                // 防溢出 clamp
                if (top < {vp_pad}) top = {vp_pad};
                if (top + bh > window.innerHeight - {vp_pad}) top = window.innerHeight - bh - {vp_pad};
                if (left < {vp_pad}) left = {vp_pad};
                if (left + bw > window.innerWidth - {vp_pad}) left = window.innerWidth - bw - {vp_pad};

                bubble.style.top = top + 'px';
                bubble.style.left = left + 'px';

                // 箭头定位：指向触发元素中心
                var arrow = bubble.querySelector('[data-tooltip="arrow"]');
                if (arrow) {{
                    var pos = bubble.getAttribute('data-pos');
                    if (pos === 'top-right' || pos === 'bottom-right') {{
                        // 箭头在气泡左侧边缘
                        var ax = cx - left - 4;
                        if (ax < 10) ax = 10;
                        if (ax > bw * 0.4) ax = bw * 0.4;
                        arrow.style.left = ax + 'px';
                        arrow.style.right = '';
                    }} else {{
                        // 箭头在气泡右侧边缘
                        var axFromRight = (left + bw) - cx - 4;
                        if (axFromRight < 10) axFromRight = 10;
                        if (axFromRight > bw * 0.4) axFromRight = bw * 0.4;
                        arrow.style.right = axFromRight + 'px';
                        arrow.style.left = '';
                    }}
                    arrow.style.transform = 'rotate(45deg)';

                    // 箭头垂直位置
                    if (pos.startsWith('top')) {{
                        arrow.style.bottom = '-5px';
                        arrow.style.top = '';
                    }} else {{
                        arrow.style.top = '-5px';
                        arrow.style.bottom = '';
                    }}
                }}

                bubble.style.visibility = 'visible';
            }}
        }}, 50);
        "#,
        gap = BUBBLE_GAP,
        vp_pad = VIEWPORT_PADDING,
    )
}

#[derive(Clone, PartialEq)]
pub enum Position {
    Top,
    Bottom,
}

impl Position {
    pub fn as_str(&self) -> &'static str {
        match self {
            Position::Top => "tooltip-top",
            Position::Bottom => "tooltip-bottom",
        }
    }
}

#[with_css(css, "styles/components/tooltip.scss")]
#[component]
pub fn Tooltip(
    text: String,
    #[props(default = Position::Top)]
    position: Position,
    children: Element,
) -> Element {
    let mut visible = use_signal(|| false);

    let show = move |_| {
        visible.set(true);
        let _ = dioxus::desktop::window().webview.evaluate_script(position_script().as_str());
    };
    let hide = move |_| {
        visible.set(false);
    };
    let toggle = move |_| {
        visible.toggle();
        if *visible.read() {
            let _ = dioxus::desktop::window().webview.evaluate_script(position_script().as_str());
        }
    };

    let pos_class = position.as_str();
    let visible_attr = if *visible.read() { "true" } else { "false" };

    rsx! {
        div {
            class: "{css::tooltip_wrapper}",
            "data-tooltip": "wrapper",
            "data-visible": "{visible_attr}",
            onmouseenter: show,
            onmouseleave: hide,
            onclick: toggle,
            {children}
            div {
                class: "{css::tooltip_bubble} {pos_class}",
                "data-tooltip": "bubble",
                role: "tooltip",
                span { "{text}" }
                div {
                    class: "{css::tooltip_arrow}",
                    "data-tooltip": "arrow",
                }
            }
        }
    }
}
