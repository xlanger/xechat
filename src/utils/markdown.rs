use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

use comrak::adapters::CodefenceRendererAdapter;
use comrak::markdown_to_html_with_plugins;
use comrak::options::Plugins;
use comrak::plugins::syntect::SyntectAdapter;
use once_cell::sync::Lazy;

use crate::icons::tabler;

/// mermaid SVG 缓存：key = 源码的 SHA-256 哈希，value = 渲染后的 SVG 字符串。
/// 避免相同流程图重复调用 headless Chrome 渲染。
static MERMAID_SVG_CACHE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// 计算字符串的简单哈希（用于缓存 key）。
fn hash_code(code: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    hasher.finish().to_string()
}

static SYNTECT_ADAPTER_DARK: Lazy<Option<SyntectAdapter>> =
    Lazy::new(|| {
        std::panic::catch_unwind(|| SyntectAdapter::new(Some("base16-ocean.dark"))).ok()
    });

static SYNTECT_ADAPTER_LIGHT: Lazy<Option<SyntectAdapter>> =
    Lazy::new(|| {
        std::panic::catch_unwind(|| SyntectAdapter::new(Some("InspiredGitHub"))).ok()
    });

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum SyntaxTheme {
    Light,
    Dark,
}

/// 构建 mermaid 渲染选项，背景设为透明，由外层 CSS 提供背景色。
fn mermaid_render_options() -> mermaid_rs_renderer::RenderOptions {
    use mermaid_rs_renderer::{Theme, RenderOptions, LayoutConfig};
    let mut theme = Theme::modern();
    theme.background = "transparent".to_string();
    RenderOptions { theme, layout: LayoutConfig::default() }
}

/// 尝试从缓存获取 mermaid SVG，成功时返回渲染后的 HTML 块。
fn try_mermaid_from_cache(key: &str, lang: &str, trimmed: &str) -> Option<String> {
    let cache = MERMAID_SVG_CACHE.lock().ok()?;
    let svg = cache.get(key)?;
    Some(build_mermaid_block(lang, svg, trimmed))
}

/// 渲染 mermaid 图表并写入缓存，返回渲染后的 HTML 块。
fn render_and_cache_mermaid(key: &str, lang: &str, trimmed: &str) -> String {
    let result = mermaid_rs_renderer::render_with_options(trimmed, mermaid_render_options())
        .or_else(|_| mermaid_rs_renderer::render(trimmed));

    match result {
        Ok(svg) => {
            if let Ok(mut cache) = MERMAID_SVG_CACHE.lock() {
                cache.insert(key.to_string(), svg.clone());
            }
            build_mermaid_block(lang, &svg, trimmed)
        }
        Err(_) => {
            let escaped = html_escape(trimmed);
            format!(
                "<div style=\"overflow-x:auto;padding:12px 16px;background:var(--bg-inset);border-radius:6px;border:1px solid var(--border)\"><pre style=\"margin:0;white-space:pre\">{escaped}</pre></div>"
            )
        }
    }
}

struct MermaidRenderer;

impl CodefenceRendererAdapter for MermaidRenderer {
    fn write(
        &self,
        output: &mut dyn fmt::Write,
        lang: &str,
        _meta: &str,
        code: &str,
        _sourcepos: Option<comrak::nodes::Sourcepos>,
    ) -> fmt::Result {
        let trimmed = code.trim_end();
        let key = hash_code(trimmed);

        let html = try_mermaid_from_cache(&key, lang, trimmed)
            .unwrap_or_else(|| render_and_cache_mermaid(&key, lang, trimmed));

        writeln!(output, "{}", html)
    }
}

/// mermaid 交互 JS 函数，需通过 `evaluate_script` 注入（`dangerous_inner_html` 中的 `<script>` 不会执行）。
static MERMAID_JS: &str = r#"
if(!window._mermaidInit){
window._mermaidInit=1;
window._mTab=function(el){var b=el.closest('.mermaid-block'),c=el.dataset.view==='chart';b.querySelectorAll('.m-tab').forEach(function(t){t.classList.remove('m-tab-active')});el.classList.add('m-tab-active');b.querySelector('.m-chart-view').style.display=c?'':'none';b.querySelector('.m-src-view').style.display=c?'none':'';b.querySelectorAll('.m-chart-act').forEach(function(a){a.style.display=c?'flex':'none'});b.querySelectorAll('.m-src-act').forEach(function(a){a.style.display=c?'none':'flex'})};
window._mGT=function(w){var t=w.style.transform||'scale(1) translate(0px,0px)',sm=t.match(/scale\(([^)]+)\)/),tm=t.match(/translate\(([^)]+)\)/),tv=tm?tm[1].split(','):['0px','0px'];return{sc:sm?parseFloat(sm[1]):1,tx:parseFloat(tv[0])||0,ty:parseFloat(tv[1])||0}};
window._mZoomIn=function(el){var w=el.closest('.mermaid-block').querySelector('.m-zoom-wrap'),g=window._mGT(w),n=Math.min(g.sc+0.2,3);w.style.transform='scale('+n+') translate('+g.tx+'px,'+g.ty+'px)'};
window._mZoomOut=function(el){var w=el.closest('.mermaid-block').querySelector('.m-zoom-wrap'),g=window._mGT(w),n=Math.max(0.4,g.sc-0.2);w.style.transform='scale('+n+') translate('+g.tx+'px,'+g.ty+'px)'};
window._mDrag=function(w){if(w.dataset.dragInit)return;w.dataset.dragInit='1';var d=false,vx=0,vy=0,sx=0,sy=0;var p=window._mGT(w);vx=p.tx;vy=p.ty;w.addEventListener('mousedown',function(e){d=true;var c=window._mGT(w);vx=c.tx;vy=c.ty;sx=e.clientX-vx;sy=e.clientY-vy;w.style.cursor='grabbing';e.preventDefault()});w.addEventListener('mousemove',function(e){if(!d)return;vx=e.clientX-sx;vy=e.clientY-sy;var c=window._mGT(w);w.style.transform='scale('+c.sc+') translate('+vx+'px,'+vy+'px)'});w.addEventListener('mouseup',function(){d=false;w.style.cursor='grab'});w.addEventListener('mouseleave',function(){d=false;w.style.cursor='grab'})};
window._mFullscreen=function(el){var src=el.closest('.mermaid-block').querySelector('.m-zoom-wrap').innerHTML,o=document.createElement('div');o.style.cssText='position:fixed;inset:0;z-index:9999;background:rgba(0,0,0,0.5);display:flex;align-items:center;justify-content:center';var box=document.createElement('div');box.style.cssText='position:relative;width:90vw;height:90vh;overflow:hidden;background:var(--bg-root);border-radius:8px;border:1px solid var(--border)';var wrap=document.createElement('div');wrap.style.cssText='transform:scale(1) translate(0px,0px);cursor:grab;display:inline-block';wrap.innerHTML=src;var svg=wrap.querySelector('svg');if(svg){svg.style.maxWidth='100%';svg.style.height='auto'}box.appendChild(wrap);var hdr=document.createElement('div');hdr.style.cssText='position:absolute;top:8px;right:8px;display:flex;gap:4px;z-index:1';var zin=document.createElement('span');zin.innerHTML="<svg viewBox='0 0 24 24' width='16' height='16'><path fill='none' stroke='currentColor' stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M3 10a7 7 0 1 0 14 0a7 7 0 1 0 -14 0m4 0h6m-3-3v6m11 8l-6-6'/></svg>";zin.style.cssText='cursor:pointer;color:var(--text-secondary);padding:1px;border-radius:4px;background:var(--bg-inset)';zin.onclick=function(){var g=window._mGT(wrap),n=Math.min(g.sc+0.2,3);wrap.style.transform='scale('+n+') translate('+g.tx+'px,'+g.ty+'px)'};var zout=document.createElement('span');zout.innerHTML="<svg viewBox='0 0 24 24' width='16' height='16'><path fill='none' stroke='currentColor' stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M7 10a3 3 0 1 0 6 0a3 3 0 1 0 -6 0m12 8l-6-6'/></svg>";zout.style.cssText='cursor:pointer;color:var(--text-secondary);padding:1px;border-radius:4px;background:var(--bg-inset)';zout.onclick=function(){var g=window._mGT(wrap),n=Math.max(0.4,g.sc-0.2);wrap.style.transform='scale('+n+') translate('+g.tx+'px,'+g.ty+'px)'};var xbtn=document.createElement('span');xbtn.innerHTML="<svg viewBox='0 0 24 24' width='16' height='16'><path fill='none' stroke='currentColor' stroke-linecap='round' stroke-linejoin='round' stroke-width='2' d='M18 6L6 18M6 6l12 12'/></svg>";xbtn.style.cssText='cursor:pointer;color:var(--text-secondary);padding:1px;border-radius:4px;background:var(--bg-inset)';xbtn.onclick=function(){o.remove()};hdr.appendChild(zin);hdr.appendChild(zout);hdr.appendChild(xbtn);box.appendChild(hdr);o.appendChild(box);o.onclick=function(e){if(e.target===o)o.remove();e.stopPropagation()};document.querySelector('.app-root').appendChild(o);window._mDrag(wrap)};
window._mCopy=function(el){var c=el.closest('.mermaid-block').querySelector('.m-src-code').textContent;var a=el.querySelector('.c-btn'),b=el.querySelector('.d-btn');if(navigator.clipboard&&window.isSecureContext){navigator.clipboard.writeText(c)}else{var t=document.createElement('textarea');t.value=c;t.style.position='fixed';t.style.left='-9999px';document.body.appendChild(t);t.select();document.execCommand('copy');document.body.removeChild(t)}a.style.display='none';b.style.display='';setTimeout(function(){a.style.display='flex';b.style.display='none'},2000)};
}
"#;

/// 获取 mermaid 交互 JS（供 Markdown 组件通过 `evaluate_script` 注入）。
pub fn mermaid_js() -> &'static str {
    MERMAID_JS
}

/// 构建 mermaid 交互式块：头部（语言标签 + 图表/源码切换 + 操作按钮）+ 图表视图 + 源码视图。
///
/// 交互通过全局 JS 函数实现（通过 `evaluate_script` 注入，`onclick` 调用 `window._mTab()` 等）。
fn build_mermaid_block(lang: &str, svg: &str, source: &str) -> String {
    let lang_display = match lang {
        "m" | "mermaidgraph" => "mermaid",
        other => other,
    };
    let escaped_source = html_escape(source);

    // Tabler Icons SVG（属性用单引号，避免截断 onclick 双引号）
    let svg_zoom_in = format!(
        "<svg viewBox='{}' width='16' height='16'>{}</svg>",
        tabler::ZoomIn.view_box, tabler::ZoomIn.body
    );
    let svg_zoom_out = format!(
        "<svg viewBox='{}' width='16' height='16'>{}</svg>",
        tabler::ZoomOut.view_box, tabler::ZoomOut.body
    );
    let svg_fullscreen = format!(
        "<svg viewBox='{}' width='16' height='16'>{}</svg>",
        tabler::Maximize.view_box, tabler::Maximize.body
    );
    let svg_copy = format!(
        "<svg viewBox='{}' width='14' height='14'>{}</svg>",
        tabler::Copy.view_box, tabler::Copy.body
    );

    let btn_base = "cursor:pointer;color:var(--text-secondary);padding:2px 4px;border-radius:4px;transition:all 0.2s;";
    let btn_flex = format!("{btn_base}display:flex;align-items:center;");

    let mut h = String::new();

    // 外层容器
    h.push_str(r#"<div class="mermaid-block" style="border-radius:8px;overflow:hidden;margin:8px 0;border:1px solid var(--border);background:var(--bg-surface)">"#);

    // 头部
    h.push_str(r#"<div style="display:flex;justify-content:space-between;align-items:center;padding:4px 8px;background:var(--bg-root);border-bottom:1px solid var(--border);font-size:12px">"#);

    // 语言标签
    h.push_str(r#"<span style="color:var(--color-accent);font-family:-apple-system,BlinkMacSystemFont,sans-serif">"#);
    h.push_str(lang_display);
    h.push_str("</span>");

    // 图表/源码切换
    h.push_str(r#"<div style="display:flex;align-items:center;gap:4px;font-family:-apple-system,BlinkMacSystemFont,sans-serif">"#);
    h.push_str(r#"<span class="m-tab m-tab-active" data-view="chart" onclick="window._mTab(this)" style="cursor:pointer;min-width:28px;text-align:center">图表</span>"#);
    h.push_str(r#"<span style="color:var(--border)">|</span>"#);
    h.push_str(r#"<span class="m-tab" data-view="source" onclick="window._mTab(this)" style="cursor:pointer;min-width:28px;text-align:center">源码</span>"#);
    h.push_str("</div>");

    // 操作按钮（固定宽度避免切换时居中偏移）
    h.push_str(r#"<div style="display:flex;gap:2px;align-items:center;min-width:84px;justify-content:flex-end">"#);
    // 图表状态按钮
    h.push_str(&format!(
        r#"<span class="m-chart-act" onclick="window._mZoomIn(this)" style="{btn_flex}">{svg_zoom_in}</span>"#
    ));
    h.push_str(&format!(
        r#"<span class="m-chart-act" onclick="window._mZoomOut(this)" style="{btn_flex}">{svg_zoom_out}</span>"#
    ));
    h.push_str(&format!(
        r#"<span class="m-chart-act" onclick="window._mFullscreen(this)" style="{btn_flex}">{svg_fullscreen}</span>"#
    ));
    // 源码状态按钮
    h.push_str(&format!(
        r#"<span class="m-src-act" style="display:none;{btn_base}" onclick="window._mCopy(this)"><span class="c-btn" style="display:flex;align-items:center;gap:2px">{svg_copy}复制</span><span class="d-btn" style="display:none">✓ 已复制</span></span>"#
    ));
    h.push_str("</div>");

    h.push_str("</div>"); // end header

    // 图表视图
    h.push_str(r#"<div class="m-chart-view" style="overflow:hidden;background:var(--bg-root);padding:8px;max-height:80vh;position:relative">"#);
    h.push_str(r#"<div class="m-zoom-wrap" style="transform:scale(1) translate(0px,0px);cursor:grab;display:inline-block" onmousedown="window._mDrag(this)">"#);
    h.push_str(svg);
    h.push_str("</div></div>");

    // 源码视图
    h.push_str(r#"<div class="m-src-view" style="display:none;padding:12px 16px;background:var(--bg-root);overflow-x:auto"><div class="m-src-code" style="margin:0;white-space:pre;font-size:14px;font-family:monospace">"#);
    h.push_str(&escaped_source);
    h.push_str("</div></div>");

    h.push_str("</div>"); // end wrapper

    h
}

static MERMAID_RENDERER: MermaidRenderer = MermaidRenderer;

pub fn render_to_html(content: &str, theme: SyntaxTheme) -> String {
    if content.is_empty() {
        return String::new();
    }

    let preprocessed = preprocess_math(content);

    let mut options = comrak::Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.math_dollars = true;
    options.extension.math_code = true;
    options.extension.superscript = true;
    options.extension.subscript = true;
    options.extension.highlight = true;
    options.extension.alerts = true;
    options.extension.description_lists = true;
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;

    let mut plugins = Plugins::default();

    let adapter = match theme {
        SyntaxTheme::Dark => &*SYNTECT_ADAPTER_DARK,
        SyntaxTheme::Light => &*SYNTECT_ADAPTER_LIGHT,
    };

    if let Some(adapter) = adapter {
        plugins.render.codefence_syntax_highlighter = Some(adapter as &dyn comrak::adapters::SyntaxHighlighterAdapter);
    }

    let mut codefence_renderers: HashMap<String, &dyn CodefenceRendererAdapter> = HashMap::new();
    codefence_renderers.insert("mermaid".to_string(), &MERMAID_RENDERER);
    codefence_renderers.insert("m".to_string(), &MERMAID_RENDERER);
    codefence_renderers.insert("mermaidgraph".to_string(), &MERMAID_RENDERER);
    plugins.render.codefence_renderers = codefence_renderers;

    let html = markdown_to_html_with_plugins(&preprocessed, &options, &plugins);
    post_process(&html)
}

fn post_process(html: &str) -> String {
    let html = process_math_code_blocks(html);
    let html = process_math_spans(&html);
    let html = process_leftover_dollars(&html);
    let html = cleanup_empty_math_delimiters(&html);
    add_code_block_styles(&html)
}

/// 查找 math code block 的闭合标签位置。
///
/// 从 `tag_end` 开始搜索 `</code></pre>`，返回 `(内容结束位置, 闭合标签长度)`。
/// 未找到时返回 `None`。
fn find_math_code_block_close(html: &str, tag_end: usize) -> Option<(usize, usize)> {
    let close_tag = "</code></pre>";
    html[tag_end..].find(close_tag).map(|i| (tag_end + i, close_tag.len()))
}

/// 渲染单个 math code block，返回 `(渲染结果, 下一个待处理位置)`。
///
/// 如果闭合标签未找到，返回 `None`。
fn render_math_code_block(html: &str, _abs_start: usize, after_marker: usize) -> Option<(String, usize)> {
    let tag_end = html[after_marker..].find('>').map(|i| after_marker + i + 1)?;

    let (content_end, close_len) = find_math_code_block_close(html, tag_end)?;

    let latex = html[tag_end..content_end].trim_end_matches('\n');
    let rendered = render_katex(latex, true);
    Some((rendered, content_end + close_len))
}

fn process_math_code_blocks(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut pos = 0;

    while pos < html.len() {
        let marker = "<pre><code class=\"language-math\"";
        if let Some(start) = html[pos..].find(marker) {
            let abs_start = pos + start;
            result.push_str(&html[pos..abs_start]);

            let after_marker = abs_start + marker.len();

            match render_math_code_block(html, abs_start, after_marker) {
                Some((rendered, next_pos)) => {
                    result.push_str(&rendered);
                    pos = next_pos;
                }
                None => {
                    result.push_str(marker);
                    pos = after_marker;
                }
            }
        } else {
            result.push_str(&html[pos..]);
            break;
        }
    }

    result
}

/// 选择两个 Option 中较近的位置（两者都存在时取较小的）。
#[inline]
fn min_span(a: (usize, bool), b: (usize, bool)) -> (usize, bool) {
    if a.0 <= b.0 { a } else { b }
}

/// 选择两个 Option 中较近的位置。
///
/// 两个都为 None 时返回 None，只有一个有时返回那个，两个都有时返回较小的。
fn pick_closer_span(inline: Option<(usize, bool)>, display: Option<(usize, bool)>) -> Option<(usize, bool)> {
    match (inline, display) {
        (Some(a), Some(b)) => Some(min_span(a, b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// 查找最近的 math span 标记位置，返回 `(标记起始位置, 是否为 display 模式)`。
///
/// 同时搜索 inline 和 display 两种标记，返回距离 `pos` 最近的一个。
fn find_next_math_span(html: &str, pos: usize) -> Option<(usize, bool)> {
    let inline_marker = "<span data-math-style=\"inline\">";
    let display_marker = "<span data-math-style=\"display\">";

    let inline_pos = html[pos..].find(inline_marker).map(|i| (pos + i, false));
    let display_pos = html[pos..].find(display_marker).map(|i| (pos + i, true));

    pick_closer_span(inline_pos, display_pos)
}

/// 处理找到闭合标签的情况：减少深度，检查是否匹配完成。
///
/// 返回 `(新深度, 是否匹配完成)`。
fn handle_close_tag(depth: usize, _close_idx: usize) -> (usize, bool) {
    let new_depth = depth - 1;
    (new_depth, new_depth == 0)
}

/// 处理找到开放标签的情况：增加深度。
fn handle_open_tag(depth: usize) -> usize {
    depth + 1
}

/// 当同时存在 open 和 close 标签时，处理 span 深度变化。
///
/// 返回 `(新深度, 下一个搜索位置, 是否匹配完成)`。
fn handle_span_with_both(
    depth: usize,
    abs_open: usize,
    close_idx: usize,
    close_tag_len: usize,
) -> (usize, usize, bool) {
    if close_idx < abs_open {
        let (new_depth, done) = handle_close_tag(depth, close_idx);
        let next_pos = if done { close_idx } else { close_idx + close_tag_len };
        (new_depth, next_pos, done)
    } else {
        (handle_open_tag(depth), abs_open + 5, false)
    }
}

/// 处理只有 close 标签（没有 open 标签）的情况。
///
/// 返回 `(新深度, 下一个搜索位置, 匹配到的闭合位置)`。
/// 若匹配完成，返回闭合位置；否则返回 None。
fn handle_close_only_span(
    depth: usize,
    search_pos: usize,
    close_idx: usize,
    close_tag_len: usize,
) -> (usize, usize, Option<usize>) {
    let abs_close = search_pos + close_idx;
    let (new_depth, done) = handle_close_tag(depth, abs_close);
    let next_pos = abs_close + close_tag_len;
    let result = if done { Some(abs_close) } else { None };
    (new_depth, next_pos, result)
}

/// 处理有 open 标签时的 span 匹配逻辑。
///
/// 返回 `(新深度, 下一个搜索位置, 匹配到的闭合位置)`。
fn process_open_tag_case(
    depth: usize,
    abs_open: usize,
    next_close: Option<usize>,
    close_tag_len: usize,
) -> (usize, usize, Option<usize>) {
    match next_close {
        Some(close_idx) => {
            let (new_depth, next_pos, done) = handle_span_with_both(
                depth, abs_open, close_idx, close_tag_len,
            );
            let result = if done { Some(close_idx) } else { None };
            (new_depth, next_pos, result)
        }
        None => {
            (handle_open_tag(depth), abs_open + 5, None)
        }
    }
}

/// 处理搜索位置中同时存在 open 和 close tag 的情况。
///
/// 返回 `(新深度, 下一个搜索位置, 可能的闭合位置)`。
fn advance_with_open_tag(
    html: &str, search_pos: usize, depth: usize, close_tag: &str,
) -> (usize, usize, Option<usize>) {
    let next_open = html[search_pos..].find("<span").unwrap();
    let abs_open = search_pos + next_open;
    let next_close = html[search_pos..].find(close_tag).map(|i| search_pos + i);
    process_open_tag_case(depth, abs_open, next_close, close_tag.len())
}

/// 处理搜索位置中只有 close tag 的情况。
///
/// 返回 `(新深度, 下一个搜索位置, 可能的闭合位置)`。
fn advance_with_close_only(
    html: &str, search_pos: usize, depth: usize, close_tag: &str,
) -> Option<(usize, usize, Option<usize>)> {
    let close_idx = html[search_pos..].find(close_tag)?;
    Some(handle_close_only_span(depth, search_pos, close_idx, close_tag.len()))
}

/// 查找与 `content_start` 位置匹配的 `</span>` 闭合标签。
///
/// 处理嵌套 `<span>` 标签，返回闭合标签的起始位置。
fn find_matching_close_span(html: &str, content_start: usize) -> Option<usize> {
    let close_tag = "</span>";
    let mut search_pos = content_start;
    let mut depth = 1;

    while depth > 0 {
        let (new_depth, next_pos, result) = if html[search_pos..].contains("<span") {
            advance_with_open_tag(html, search_pos, depth, close_tag)
        } else {
            advance_with_close_only(html, search_pos, depth, close_tag)?
        };
        depth = new_depth;
        if let Some(close_pos) = result {
            return Some(close_pos);
        }
        search_pos = next_pos;
    }

    None
}

/// 渲染单个 math span 标记，返回 `(渲染结果, 下一个待处理位置)`。
fn render_math_span(html: &str, span_start: usize, is_display: bool) -> (String, usize) {
    let inline_marker = "<span data-math-style=\"inline\">";
    let display_marker = "<span data-math-style=\"display\">";
    let marker = if is_display { display_marker } else { inline_marker };
    let content_start = span_start + marker.len();
    let close_tag = "</span>";

    match find_matching_close_span(html, content_start) {
        Some(end) => {
            let latex = &html[content_start..end];
            let rendered = render_katex(latex, is_display);
            (rendered, end + close_tag.len())
        }
        None => (marker.to_string(), content_start),
    }
}

fn process_math_spans(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut pos = 0;

    while pos < html.len() {
        match find_next_math_span(html, pos) {
            Some((span_start, is_display)) => {
                result.push_str(&html[pos..span_start]);
                let (rendered, next_pos) = render_math_span(html, span_start, is_display);
                result.push_str(&rendered);
                pos = next_pos;
            }
            None => {
                result.push_str(&html[pos..]);
                break;
            }
        }
    }

    result
}

fn cleanup_empty_math_delimiters(html: &str) -> String {
    let mut result = html.to_string();
    result = result.replace("\n$$\n<p>$$</p>", "\n");
    result = result.replace("<p>$$</p>", "");
    result = result.replace("$$\n\n$$", "\n\n");
    loop {
        let prev = result.clone();
        result = result.replace("<p>$$", "<p>");
        result = result.replace("$$</p>", "</p>");
        if result == prev {
            break;
        }
    }
    result = result.replace("\n$$ ", "\n");
    result = result.replace(" $$\n", "\n");
    result
}

/// 处理转义字符，返回 `(新位置, 是否遇到提前终止符)`。
///
/// 遇到 `\)` 或 `\]` 时返回 `(转义起始位置, true)` 表示提前终止；
/// 否则跳过转义字符及后一个字符，返回 `(新位置, false)`。
fn handle_escape_in_scan(chars: &[char], pos: usize) -> (usize, bool) {
    let saved = pos;
    let pos = pos + 1;
    if pos < chars.len() && (chars[pos] == ')' || chars[pos] == ']') {
        return (saved, true);
    }
    let new_pos = if pos < chars.len() { pos + 1 } else { pos };
    (new_pos, false)
}

/// 处理扫描中遇到的花括号字符。
///
/// 返回 `(新位置, 是否找到闭合)`。
fn scan_brace_char(depth: &mut usize, c: char, pos: usize) -> (usize, bool) {
    match c {
        '{' => { *depth += 1; (pos + 1, false) }
        '}' => { if *depth > 0 { *depth -= 1; } (pos + 1, false) }
        _ => unreachable!(),
    }
}

/// 处理扫描中遇到的 `$` 字符。
///
/// depth 为 0 时返回闭合位置，否则跳过。
fn scan_dollar_char(depth: usize, pos: usize) -> (usize, bool) {
    if depth == 0 {
        (pos, true)
    } else {
        (pos + 1, false)
    }
}

/// 扫描结果的下一步动作。
enum ScanAction {
    /// 继续扫描，移动到指定位置。
    Continue(usize),
    /// 找到闭合标记，在指定位置。
    Found(usize),
    /// 提前终止，在指定位置。
    EarlyExit(usize),
}

/// 处理扫描中遇到的转义字符。
fn handle_scan_escape(chars: &[char], pos: usize) -> ScanAction {
    let (new_pos, early_exit) = handle_escape_in_scan(chars, pos);
    if early_exit { ScanAction::EarlyExit(new_pos) } else { ScanAction::Continue(new_pos) }
}

/// 处理扫描中遇到的 `$` 字符。
fn handle_scan_dollar(depth: usize, pos: usize) -> ScanAction {
    let (new_pos, found) = scan_dollar_char(depth, pos);
    if found { ScanAction::Found(new_pos) } else { ScanAction::Continue(new_pos) }
}

/// 处理扫描中遇到的单个字符。
fn handle_scan_char(chars: &[char], depth: &mut usize, pos: usize) -> ScanAction {
    match chars[pos] {
        '{' | '}' => {
            let (new_pos, _) = scan_brace_char(depth, chars[pos], pos);
            ScanAction::Continue(new_pos)
        }
        '\\' => handle_scan_escape(chars, pos),
        '$' => handle_scan_dollar(*depth, pos),
        _ => ScanAction::Continue(pos + 1),
    }
}

/// 扫描行内公式的闭合 `$`，返回 `(闭合位置, 是否找到)`。
///
/// 从 `start + 1` 开始，跳过花括号分组，遇到未转义的 `\)` 或 `\]` 提前终止。
pub fn scan_inline_closing(chars: &[char], start: usize) -> (usize, bool) {
    let mut pos = start + 1;
    let mut depth = 0;

    while pos < chars.len() {
        match handle_scan_char(chars, &mut depth, pos) {
            ScanAction::Continue(new_pos) => pos = new_pos,
            ScanAction::Found(end_pos) => return (end_pos, true),
            ScanAction::EarlyExit(exit_pos) => return (exit_pos, false),
        }
    }
    (pos, false)
}

/// 判断 LaTeX 内容是否可安全渲染（非空且不含 HTML 标签）。
pub fn is_renderable_latex(content: &str) -> bool {
    let trimmed = content.trim();
    !trimmed.is_empty() && !trimmed.contains('<') && !trimmed.contains('>')
}

/// 处理单 `$...$` 行内公式：从 `start` 位置（`$` 所在位置）开始，
/// 尝试找到匹配的闭合 `$`，成功则渲染 KaTeX，失败则原样输出。
/// 返回 `(写入 result 的内容, 下一个待处理位置)`。
fn process_inline_math(chars: &[char], start: usize) -> (String, usize) {
    let (end_pos, found_end) = scan_inline_closing(chars, start);

    if found_end {
        let latex: String = chars[start + 1..end_pos].iter().collect();
        if is_renderable_latex(&latex) {
            let rendered = render_katex(latex.trim(), false);
            (rendered, end_pos + 1)
        } else {
            let original: String = chars[start..=end_pos].iter().collect();
            (original, end_pos + 1)
        }
    } else {
        let original: String = chars[start..end_pos].iter().collect();
        (original, end_pos)
    }
}

/// 扫描 `$$` 闭合标记，返回 `(闭合位置, 是否找到)`。
pub fn scan_display_closing(chars: &[char], start: usize) -> (usize, bool) {
    let mut pos = start + 2;
    while pos + 1 < chars.len() {
        if chars[pos] == '$' && chars[pos + 1] == '$' {
            return (pos, true);
        }
        pos += 1;
    }
    (pos, false)
}

/// 判断无闭合 `$$` 的尾部内容是否含有 LaTeX 特征字符。
pub fn has_latex_features(content: &str) -> bool {
    content.contains('\\') || content.contains('_') || content.contains('^')
}

/// 渲染已闭合的 display math 内容，返回 `(渲染结果, 下一个位置)`。
fn render_closed_display(chars: &[char], start: usize, end_pos: usize) -> (String, usize) {
    let raw_latex: String = chars[start + 2..end_pos].iter().collect();
    let cleaned = strip_all_html_tags(&raw_latex);
    let trimmed = cleaned.trim();
    if !trimmed.is_empty() {
        (render_katex(trimmed, true), end_pos + 2)
    } else {
        let original: String = chars[start..=end_pos + 1].iter().collect();
        (original, end_pos + 2)
    }
}

/// 渲染未闭合的 display math 尾部内容，返回 `(渲染结果, 下一个位置)`。
fn render_unclosed_display(chars: &[char], start: usize) -> (String, usize) {
    let raw_latex: String = chars[start + 2..].iter().collect();
    let cleaned = strip_all_html_tags(&raw_latex);
    let trimmed = cleaned.trim();
    if !trimmed.is_empty() && has_latex_features(trimmed) {
        (render_katex(trimmed, true), chars.len())
    } else {
        let original: String = chars[start..].iter().collect();
        (original, chars.len())
    }
}

/// 处理双 `$$...$$` 独占行公式：从 `start` 位置（第一个 `$` 所在位置）开始，
/// 尝试找到匹配的闭合 `$$`，成功则渲染 KaTeX，失败则扫描到段落末尾。
/// 返回 `(写入 result 的内容, 下一个待处理位置)`。
fn process_display_math(chars: &[char], start: usize) -> (String, usize) {
    let (end_pos, found_closing) = scan_display_closing(chars, start);

    if found_closing {
        render_closed_display(chars, start, end_pos)
    } else {
        render_unclosed_display(chars, start)
    }
}

/// 判断当前位置是否为双美元符号 `$$`。
fn is_ddollar(chars: &[char], pos: usize) -> bool {
    chars[pos] == '$' && pos + 1 < chars.len() && chars[pos + 1] == '$'
}

/// 判断当前位置是否为单美元符号 `$`（非 `$$`）。
fn is_single_dollar(chars: &[char], pos: usize) -> bool {
    chars[pos] == '$' && (pos + 1 >= chars.len() || chars[pos + 1] != '$')
}

fn process_leftover_dollars(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let chars: Vec<char> = html.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        if is_ddollar(&chars, pos) {
            let (text, next) = process_display_math(&chars, pos);
            result.push_str(&text);
            pos = next;
        } else if is_single_dollar(&chars, pos) {
            let (text, next) = process_inline_math(&chars, pos);
            result.push_str(&text);
            pos = next;
        } else {
            result.push(chars[pos]);
            pos += 1;
        }
    }

    result
}

fn render_katex(latex: &str, display_mode: bool) -> String {
    let opts = katex::Opts::builder()
        .display_mode(display_mode)
        .output_type(katex::OutputType::Html)
        .build()
        .unwrap();

    match katex::render_with_opts(latex, &opts) {
        Ok(html) => html,
        Err(_) => {
            let escaped = html_escape(latex);
            format!(
                "<span style=\"border:1px solid #e74c3c;color:#e74c3c;padding:2px 6px;border-radius:3px;font-family:monospace;font-size:0.9em\">{escaped}</span>"
            )
        }
    }
}

/// 查找 `<pre>` 标签的结束位置和闭合标签位置。
///
/// 返回 `(标签结束位置, pre属性, 代码内容, 内容结束位置)`。
/// 任何一步未找到时返回 `None`。
fn parse_pre_block(html: &str, abs_start: usize) -> Option<(usize, &str, &str, usize)> {
    let tag_end = html[abs_start..].find('>').map(|i| abs_start + i + 1)?;

    let pre_attrs = &html[abs_start + 4..tag_end - 1];

    let close_pre = "</pre>";
    let content_end = html[tag_end..].find(close_pre).map(|i| tag_end + i)?;

    let code_content = &html[tag_end..content_end];

    Some((tag_end, pre_attrs, code_content, content_end))
}

fn add_code_block_styles(html: &str) -> String {
    let mut result = String::with_capacity(html.len() * 2);
    let mut pos = 0;

    while pos < html.len() {
        let pre_marker = "<pre";
        if let Some(start) = html[pos..].find(pre_marker) {
            let abs_start = pos + start;
            result.push_str(&html[pos..abs_start]);

            match parse_pre_block(html, abs_start) {
                Some((_tag_end, pre_attrs, code_content, content_end)) => {
                    let lang = extract_language_from_code_tag(code_content)
                        .or_else(|| extract_language_from_attrs(pre_attrs));
                    let lang_display = lang.as_deref().unwrap_or("");
                    let wrapper = build_code_block_wrapper(lang_display, pre_attrs, code_content);
                    result.push_str(&wrapper);
                    pos = content_end + "</pre>".len();
                }
                None => {
                    result.push_str(&html[abs_start..]);
                    break;
                }
            }
        } else {
            result.push_str(&html[pos..]);
            break;
        }
    }

    result
}

fn extract_language_from_code_tag(code_content: &str) -> Option<String> {
    let code_marker = "<code class=\"language-";
    if let Some(start) = code_content.find(code_marker) {
        let value_start = start + code_marker.len();
        if let Some(value_end) = code_content[value_start..].find('"') {
            return Some(code_content[value_start..value_start + value_end].to_string());
        }
    }
    None
}

fn extract_language_from_attrs(attrs: &str) -> Option<String> {
    let class_marker = "class=\"";
    if let Some(class_start) = attrs.find(class_marker) {
        let value_start = class_start + class_marker.len();
        if let Some(value_end) = attrs[value_start..].find('"') {
            let class_value = &attrs[value_start..value_start + value_end];
            if let Some(lang) = class_value.strip_prefix("language-") {
                return Some(lang.to_string());
            }
        }
    }
    None
}

fn build_code_block_wrapper(lang: &str, pre_attrs: &str, code_content: &str) -> String {
    let mut wrapper = String::new();
    let svg_copy =  format!(r#"<svg viewBox="0 0 24 24" width="12" height="12" xmlns="http://www.w3.org/2000/svg">{}</svg>"#, tabler::Copy.body);

    wrapper.push_str("<div style=\"border-radius:8px;overflow:hidden;margin:8px 0;border:1px solid var(--border);background:var(--bg-surface);\">");
    wrapper.push_str("<div style=\"display:flex;justify-content:space-between;align-items:center;padding:2px 2px;background:var(--bg-root);border-bottom:1px solid var(--border);font-size:12px\">");
    wrapper.push_str("<span style=\"padding:0 4px;color:var(--color-accent);font-family:-apple-system,BlinkMacSystemFont,sans-serif\">");
    wrapper.push_str(lang);
    wrapper.push_str("</span>");

    wrapper.push_str("<span onclick=\"var c=this.parentElement.nextElementSibling.textContent;navigator.clipboard.writeText(c);var a=this.querySelector('.c-btn');var b=this.querySelector('.d-btn');a.style.display='none';b.style.display='';var s=this;setTimeout(function(){a.style.display='flex';b.style.display='none';},2000)\" style=\"cursor:pointer;color:var(--text-secondary);font-size:12px;font-family:-apple-system,BlinkMacSystemFont,sans-serif;padding:2px 4px;border-radius:6px;transition:all 0.2s\" onmouseover=\"this.style.background='var(--bg-inset)'\" onmouseout=\"this.style.background=''\">");

    wrapper.push_str(&format!("<span class=\"c-btn\" style=\"display:flex;align-items:center;gap:2px;\">{}\u{590d}\u{5236}</span>", svg_copy));
    wrapper.push_str("<span class=\"d-btn\" style=\"display:none\">\u{2713} \u{5df2}\u{590d}\u{5236}</span>");
    wrapper.push_str("</span>");

    wrapper.push_str("</div>");

    // 移除 syntect 自带的内联背景色样式
    let pre_attrs = pre_attrs.replace("background-color:", "background-color-disabled:");

    let styled_pre_attrs = if pre_attrs.contains("style=\"") {
        pre_attrs.replace(
            "style=\"",
            "style=\"padding:2px 6px;overflow-x:auto;margin:0;white-space:pre;tab-size:4;font-size:14px;background:transparent!important;",
        )
    } else {
        format!("{} style=\"padding:2px 6px;overflow-x:auto;margin:0;white-space:pre;tab-size:4;font-size:14px;background:transparent!important\"", pre_attrs)
    };

    // 移除 code 标签上的内联背景色，并确保 white-space/word-break 正确
    let clean_code_content = code_content
        .replace("background-color:", "background-color-disabled:")
        // 给 <code> 标签添加 white-space:pre 等样式，确保代码块换行/缩进正确
        // 注意顺序：先处理已有 style 的，再处理有 class 的（syntect 常见输出），最后处理无属性的
        .replace("<code style=\"", "<code style=\"white-space:pre;word-break:normal;overflow-wrap:normal;")
        .replace("<code class=", "<code style=\"white-space:pre;word-break:normal;overflow-wrap:normal\" class=")
        .replace("<code>", "<code style=\"white-space:pre;word-break:normal;overflow-wrap:normal\">")
        ;

    wrapper.push_str("<pre ");
    wrapper.push_str(&styled_pre_attrs);
    wrapper.push('>');
    wrapper.push_str(&clean_code_content);
    wrapper.push_str("</pre>");

    wrapper.push_str("</div>");

    wrapper
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 处理 HTML 标签剥离中的单个字符。
///
/// 返回 `(是否在标签内, 是否应将字符加入结果)`。
fn process_strip_char(c: char, inside_tag: bool) -> (bool, bool) {
    match c {
        '<' => (true, false),
        '>' => (false, false),
        _ => (inside_tag, !inside_tag),
    }
}

/// 清理 HTML 上标/下标标签为数学符号。
fn cleanup_math_tags(s: &str) -> String {
    s.replace("<sup>", "^").replace("</sup>", "")
     .replace("<sub>", "_").replace("</sub>", "")
}

fn strip_all_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut inside_tag = false;
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let (new_inside, should_push) = process_strip_char(chars[i], inside_tag);
        inside_tag = new_inside;
        if should_push {
            result.push(chars[i]);
        }
        i += 1;
    }
    cleanup_math_tags(&result)
}

/// 尝试转换数学定界符（`\[...\]`、`\(...\)`、`$$...$$`、`$...$`）。
///
/// 依次尝试每种定界符，成功转换时返回 `true`。
/// 所有定界符都不匹配时返回 `false`。
fn try_convert_math_delimiter(
    chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize,
) -> bool {
    if try_convert_bracket_display(chars, pos, result, next_pos) {
        return true;
    }
    if try_convert_paren_inline(chars, pos, result, next_pos) {
        return true;
    }
    if try_convert_ddollar_display(chars, pos, result, next_pos) {
        return true;
    }
    if try_convert_dollar_inline(chars, pos, result, next_pos) {
        return true;
    }
    false
}

/// 数学公式预处理：统一定界符并展平换行符。
///
/// - `\(...\)` → `$...$`（行内公式）
/// - `\[...\]` → `$$...$$`（独占行公式）
/// - `$...$` / `$$...$$` 内部换行符展平为空格，防止 comrak 段落解析截断超长公式
///
/// **跳过代码块和行内代码**：代码块（```...```）和行内代码（`...`）中的
/// `$`、`\[`、`\(` 等符号不进行数学公式处理，避免破坏代码内容。
pub fn preprocess_math(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        if try_skip_triple_backtick(&chars, pos, &mut result, &mut pos) {
            continue;
        }
        if try_skip_inline_code(&chars, pos, &mut result, &mut pos) {
            continue;
        }
        if try_convert_math_delimiter(&chars, pos, &mut result, &mut pos) {
            continue;
        }

        result.push(chars[pos]);
        pos += 1;
    }

    result
}

/// 判断当前位置是否为三反引号起始标记。
fn is_triple_backtick(chars: &[char], pos: usize) -> bool {
    chars[pos] == '`' && pos + 2 < chars.len() && chars[pos + 1] == '`' && chars[pos + 2] == '`'
}

/// 将字符范围写入 result 并更新 next_pos。
fn push_char_range(chars: &[char], start: usize, end: usize, result: &mut String, next_pos: &mut usize) {
    for &c in &chars[start..end] {
        result.push(c);
    }
    *next_pos = end;
}

/// 尝试跳过三反引号代码块（```...```）。
///
/// 成功跳过时返回 `true`，并将代码块原样写入 `result`，更新 `pos`。
/// 未遇到三反引号时返回 `false`，不修改任何状态。
#[inline]
pub fn try_skip_triple_backtick(
    chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize,
) -> bool {
    if !is_triple_backtick(chars, pos) {
        return false;
    }
    if let Some(end) = find_closing_triple_backtick(chars, pos + 3) {
        push_char_range(chars, pos, end + 3, result, next_pos);
    } else {
        push_char_range(chars, pos, chars.len(), result, next_pos);
    }
    true
}

/// 查找行内代码的闭合反引号位置。
///
/// 从 `start` 开始搜索，返回闭合反引号的位置；未找到则返回 `None`。
fn find_closing_backtick(chars: &[char], start: usize) -> Option<usize> {
    let mut end = start;
    while end < chars.len() && chars[end] != '`' {
        end += 1;
    }
    if end < chars.len() { Some(end) } else { None }
}

/// 判断当前位置是否为三反引号起始（排除行内代码）。
fn is_triple_backtick_excluding_inline(chars: &[char], pos: usize) -> bool {
    pos + 2 < chars.len() && chars[pos + 1] == '`' && chars[pos + 2] == '`'
}

/// 将行内代码范围写入 result 并更新 next_pos。
fn write_inline_code_range(chars: &[char], start: usize, end: usize, result: &mut String, next_pos: &mut usize) {
    for &c in &chars[start..=end] {
        result.push(c);
    }
    *next_pos = end + 1;
}

/// 将未闭合的行内代码尾部写入 result 并更新 next_pos。
fn write_unclosed_inline_code(chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize) {
    for &c in &chars[pos..] {
        result.push(c);
    }
    *next_pos = chars.len();
}

/// 尝试跳过行内代码（`...`）。
///
/// 成功跳过时返回 `true`，并将行内代码原样写入 `result`，更新 `pos`。
/// 当前位置不是反引号或未找到闭合时返回 `false`。
#[inline]
pub fn try_skip_inline_code(
    chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize,
) -> bool {
    if chars[pos] != '`' {
        return false;
    }
    // 排除三反引号（由 try_skip_triple_backtick 处理）
    if is_triple_backtick_excluding_inline(chars, pos) {
        return false;
    }
    match find_closing_backtick(chars, pos + 1) {
        Some(end) => write_inline_code_range(chars, pos, end, result, next_pos),
        None => write_unclosed_inline_code(chars, pos, result, next_pos),
    }
    true
}

/// 尝试转换 `\[...\]` 为 `$$...$$`（独占行公式）。
///
/// 成功转换时返回 `true`，将内容展平换行后写入 `result`，更新 `pos`。
/// 当前位置不是 `\[` 时返回 `false`。
#[inline]
pub fn try_convert_bracket_display(
    chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize,
) -> bool {
    if chars[pos] != '\\' || pos + 1 >= chars.len() || chars[pos + 1] != '[' {
        return false;
    }
    if let Some(end) = find_closing_pair(chars, pos + 2, '\\', ']') {
        let content = flatten_newlines(&chars[pos + 2..end]);
        result.push_str("$$");
        result.push_str(content.trim());
        result.push_str("$$");
        *next_pos = end + 2;
    } else {
        result.push_str("\\[");
        *next_pos = pos + 2;
    }
    true
}

/// 尝试转换 `\(...\)` 为 `$...$`（行内公式）。
///
/// 成功转换时返回 `true`，将内容展平换行后写入 `result`，更新 `pos`。
/// 当前位置不是 `\(` 时返回 `false`。
#[inline]
pub fn try_convert_paren_inline(
    chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize,
) -> bool {
    if chars[pos] != '\\' || pos + 1 >= chars.len() || chars[pos + 1] != '(' {
        return false;
    }
    if let Some(end) = find_closing_pair(chars, pos + 2, '\\', ')') {
        let content = flatten_newlines(&chars[pos + 2..end]);
        result.push('$');
        result.push_str(content.trim());
        result.push('$');
        *next_pos = end + 2;
    } else {
        result.push_str("\\(");
        *next_pos = pos + 2;
    }
    true
}

/// 尝试转换 `$$...$$` 独占行公式。
///
/// 成功转换时返回 `true`，将内容展平换行后写入 `result`，更新 `pos`。
/// 当前位置不是 `$$` 时返回 `false`。
#[inline]
pub fn try_convert_ddollar_display(
    chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize,
) -> bool {
    if chars[pos] != '$' || pos + 1 >= chars.len() || chars[pos + 1] != '$' {
        return false;
    }
    if let Some(end) = find_closing_ddollar(chars, pos + 2) {
        let content = flatten_newlines(&chars[pos + 2..end]);
        result.push_str("$$");
        result.push_str(content.trim());
        result.push_str("$$");
        *next_pos = end + 2;
    } else {
        result.push_str("$$");
        *next_pos = pos + 2;
    }
    true
}

/// 尝试转换 `$...$` 行内公式。
///
/// 成功转换时返回 `true`，将内容展平换行后写入 `result`，更新 `pos`。
/// 当前位置不是单个 `$`（非 `$$`）时返回 `false`。
#[inline]
pub fn try_convert_dollar_inline(
    chars: &[char], pos: usize, result: &mut String, next_pos: &mut usize,
) -> bool {
    if chars[pos] != '$' {
        return false;
    }
    // 排除 $$（由 try_convert_ddollar_display 处理）
    if pos + 1 < chars.len() && chars[pos + 1] == '$' {
        return false;
    }
    if let Some(end) = find_closing_dollar(chars, pos + 1) {
        let content = flatten_newlines(&chars[pos + 1..end]);
        result.push('$');
        result.push_str(content.trim());
        result.push('$');
        *next_pos = end + 1;
    } else {
        result.push('$');
        *next_pos = pos + 1;
    }
    true
}

fn find_closing_triple_backtick(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 2 < chars.len() {
        if chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_closing_pair(chars: &[char], start: usize, c1: char, c2: char) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == c1 && chars[i + 1] == c2 {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_closing_ddollar(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '$' && chars[i + 1] == '$' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_closing_dollar(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    while i < chars.len() {
        if chars[i] == '$' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn flatten_newlines(chars: &[char]) -> String {
    chars.iter().map(|&c| if c == '\n' { ' ' } else { c }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_macro_in_code_block() {
        // #[component] 在代码块中应完整保留
        let input = "```rust\n#[component]\nfn MyComp() -> Element {\n    rsx! {}\n}\n```";
        let preprocessed = preprocess_math(input);
        assert!(preprocessed.contains("#[component]"), "preprocess_math should not touch #[component], got: {}", preprocessed);
        // syntect 高亮会把 #[component] 拆成多个 span（如 <span>#[</span><span>component</span><span>]</span>）
        // 所以检查 HTML 文本中是否包含 # 和 [ 和 component
        let html = render_to_html(input, SyntaxTheme::Dark);
        // 去掉 HTML 标签后检查纯文本
        let text = strip_all_html_tags(&html);
        assert!(text.contains("#[component]"), "#[component] should appear in rendered code block text, got: {}", &text[..text.len().min(500)]);
    }

    #[test]
    fn test_attribute_macro_in_unclosed_code_block() {
        // 流式输出时代码块可能未闭合
        let input = "```rust\n#[component]\nfn MyComp()";
        let preprocessed = preprocess_math(input);
        assert!(preprocessed.contains("#[component]"), "preprocess_math should not touch #[component] in unclosed code block, got: {}", preprocessed);
        let html = render_to_html(input, SyntaxTheme::Dark);
        let text = strip_all_html_tags(&html);
        assert!(text.contains("#[component]"), "#[component] should appear in unclosed code block text, got: {}", &text[..text.len().min(500)]);
    }

    #[test]
    fn test_preprocess_math_preserves_code_blocks() {
        // preprocess_math 不应修改代码块内的任何内容
        let input = "```rust\nlet x = $5 + $3;\n#[derive(Clone)]\nstruct Foo;\n```";
        let result = preprocess_math(input);
        assert!(result.contains("#[derive(Clone)]"), "preprocess_math should not modify code block content, got: {}", result);
        assert!(result.contains("$5 + $3"), "preprocess_math should not modify $ in code block, got: {}", result);
    }

    #[test]
    fn test_code_block_indentation_preserved() {
        // 代码块中的缩进和换行应完整保留
        let input = "```rust\nfn main() {\n    let x = 1;\n    if x > 0 {\n        println!(\"hello\");\n    }\n}\n```";
        let preprocessed = preprocess_math(input);
        assert!(preprocessed.contains("    let x = 1;"), "indentation should be preserved in preprocess_math, got: {}", preprocessed);
        assert!(preprocessed.contains("        println!"), "nested indentation should be preserved, got: {}", preprocessed);
        let html = render_to_html(input, SyntaxTheme::Dark);
        let text = strip_all_html_tags(&html);
        // 检查缩进是否在渲染后的文本中保留
        assert!(text.contains("    let x = 1;") || text.contains("  let x = 1;"), "indentation should be preserved in rendered output, got: {}", &text[..text.len().min(500)]);
    }

    #[test]
    fn test_code_block_newlines_preserved() {
        // 代码块中的换行应保留
        let input = "```rust\nline1\nline2\nline3\n```";
        let preprocessed = preprocess_math(input);
        assert!(preprocessed.contains("line1\nline2\nline3"), "newlines should be preserved in preprocess_math, got: {}", preprocessed);
    }

    #[test]
    fn test_attribute_macro_in_inline_code() {
        // `#[component]` 行内代码中应完整保留
        let input = "Use `#[component]` to annotate.";
        let html = render_to_html(input, SyntaxTheme::Dark);
        assert!(html.contains("#[component]"), "#[component] should be preserved in inline code, got: {}", &html[..html.len().min(500)]);
    }

    #[test]
    fn test_attribute_macro_outside_code() {
        // 代码块外的 #[component] 可能被 Markdown 解析（这是预期行为）
        let input = "#[component]\nfn MyComp() -> Element {}";
        let html = render_to_html(input, SyntaxTheme::Dark);
        // 只要不 panic 就行，具体渲染结果由 comrak 决定
        assert!(!html.is_empty());
    }

    #[test]
    fn test_mermaid_render_flowchart() {
        let input = "```mermaid\nflowchart LR\n    A-->B-->C\n```";
        let html = render_to_html(input, SyntaxTheme::Dark);
        assert!(html.contains("<svg"), "Should render SVG, got: {}", &html[..html.len().min(500)]);
        assert!(html.contains("mermaid-block"), "Should have mermaid-block wrapper");
        assert!(html.contains("m-chart-view"), "Should have chart view");
        assert!(html.contains("m-src-view"), "Should have source view");
    }

    #[test]
    fn test_mermaid_not_rendered_as_code_block() {
        let input = "```mermaid\nflowchart LR\n    A-->B\n```";
        let html = render_to_html(input, SyntaxTheme::Dark);
        assert!(!html.contains("class=\"language-mermaid\""), "Should not be code block, got: {}", &html[..html.len().min(500)]);
    }

    #[test]
    fn test_mermaid_short_lang_tag() {
        // LLM 有时返回 ```m 而不是 ```mermaid
        let input = "```m\nflowchart LR\n    A-->B\n```";
        let html = render_to_html(input, SyntaxTheme::Dark);
        assert!(html.contains("<svg"), "```m should render SVG, got: {}", &html[..html.len().min(500)]);
        assert!(html.contains(">mermaid<"), "```m should display as 'mermaid' in header");
    }

    #[test]
    fn test_mermaid_interactive_elements() {
        let input = "```mermaid\nflowchart LR\n    A-->B\n```";
        let html = render_to_html(input, SyntaxTheme::Dark);
        assert!(html.contains("m-tab-active"), "Should have active tab");
        assert!(html.contains("图表"), "Should have chart tab");
        assert!(html.contains("源码"), "Should have source tab");
        assert!(html.contains("m-chart-act"), "Should have chart action buttons");
        assert!(html.contains("m-src-act"), "Should have source action buttons");
        assert!(html.contains("m-zoom-wrap"), "Should have zoom wrapper");
    }

    #[test]
    fn test_mermaid_no_script_tag() {
        let input = "```mermaid\nflowchart LR\n    A-->B\n```";
        let html = render_to_html(input, SyntaxTheme::Dark);
        assert!(!html.contains("<script"), "Should NOT contain <script> tag (won't execute via dangerous_inner_html)");
    }

    #[test]
    fn test_mermaid_in_full_message() {
        let input = "流程图：\n\n```mermaid\nflowchart LR\n    A-->B\n```\n\n上面的图展示了流程。";
        let html = render_to_html(input, SyntaxTheme::Dark);
        assert!(html.contains("<svg"), "Should render SVG in full message, got: {}", &html[..html.len().min(800)]);
    }

    #[test]
    fn test_mermaidgraph_lang_tag() {
        // LLM 有时返回 ```mermaidgraph（无换行）
        let input = "```mermaidgraph\ngraph TB\n    A-->B\n```";
        let html = render_to_html(input, SyntaxTheme::Dark);
        // 检查是否正确触发了 mermaid renderer（而非代码块高亮）
        assert!(html.contains("mermaid-block"), "Should use mermaid renderer, got: {}", &html[..html.len().min(500)]);
        assert!(html.contains("<svg"), "```mermaidgraph should render SVG, got: {}", &html[..html.len().min(500)]);
        assert!(html.contains("m-chart-view"), "Should have chart view");
        assert!(html.contains("m-src-view"), "Should have source view");
        assert!(html.contains("graph TB"), "Source view should contain 'graph TB', got: {}", &html[..html.len().min(500)]);
    }
}
