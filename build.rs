//! 构建脚本：编译全局 SCSS 文件为 `global.css` 并注入 KaTeX CSS。

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let styles_dir = manifest_dir.join("src/styles");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let files = discover_scss_files(&styles_dir);
    let file_count = files.len();
    let mut combined = compile_global_scss(&files, &styles_dir, cfg!(not(debug_assertions)));
    append_katex_css(&mut combined, &manifest_dir);
    let css_size = combined.len();
    write_global_css(&combined, &out_dir.join("global.css")).unwrap();

    println!(
        "cargo:warning=compiled {} SCSS files + KaTeX CSS → global.css ({} bytes)",
        file_count, css_size
    );

    println!("cargo:rerun-if-changed=src/styles/");
    println!("cargo:rerun-if-changed=assets/katex/katex-inline.min.css");
}

/// 扫描 `styles_dir` 顶层 `.scss` 文件（排除子目录），按路径升序返回。
fn discover_scss_files(styles_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(styles_dir)
        .unwrap_or_else(|e| panic!("读取 styles 目录失败 {}: {}", styles_dir.display(), e))
    {
        let entry = entry.unwrap_or_else(|e| panic!("读取目录条目失败: {}", e));
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("scss") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// 编译 SCSS 文件并合并为单个 CSS 字符串；`is_release` 为真时使用 Compressed 输出。
fn compile_global_scss(files: &[PathBuf], styles_dir: &Path, is_release: bool) -> String {
    let output_style = if is_release {
        grass::OutputStyle::Compressed
    } else {
        grass::OutputStyle::Expanded
    };
    let options = grass::Options::default()
        .style(output_style)
        .load_path(styles_dir);

    let mut combined = String::new();
    for file in files {
        let content =
            fs::read_to_string(file).unwrap_or_else(|_| panic!("读取失败: {}", file.display()));
        let css = grass::from_string(content, &options)
            .unwrap_or_else(|e| panic!("SCSS 编译失败 {}: {}", file.display(), e));
        combined.push_str(&css);
        combined.push('\n');
    }
    combined
}

/// 若 KaTeX CSS 文件存在，则追加到 `css` 末尾。
fn append_katex_css(css: &mut String, manifest_dir: &Path) {
    let katex_css_path = manifest_dir.join("assets/katex/katex-inline.min.css");
    if katex_css_path.exists() {
        let katex_css = fs::read_to_string(&katex_css_path)
            .unwrap_or_else(|_| panic!("读取 KaTeX CSS 失败"));
        css.push_str(&katex_css);
        css.push('\n');
    }
}

/// 将合并后的 CSS 写入 `output` 文件。
fn write_global_css(content: &str, output: &Path) -> io::Result<()> {
    fs::write(output, content)
}
