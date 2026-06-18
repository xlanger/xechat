use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let styles_dir = manifest_dir.join("src/styles");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Auto-discover top-level .scss files in src/styles/ (deterministic order).
    let mut global_files: Vec<String> = Vec::new();
    for entry in fs::read_dir(&styles_dir)
        .unwrap_or_else(|e| panic!("读取 styles 目录失败 {}: {}", styles_dir.display(), e))
    {
        let entry = entry.unwrap_or_else(|e| panic!("读取目录条目失败: {}", e));
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("scss") {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                global_files.push(name.to_string());
            }
        }
    }
    global_files.sort();

    let output_style = if cfg!(not(debug_assertions)) {
        grass::OutputStyle::Compressed
    } else {
        grass::OutputStyle::Expanded
    };
    let options = grass::Options::default()
        .style(output_style)
        .load_path(&styles_dir);

    let mut combined = String::new();
    for file in &global_files {
        let path = styles_dir.join(file);
        let content =
            fs::read_to_string(&path).unwrap_or_else(|_| panic!("读取失败: {}", file));
        let css = grass::from_string(content, &options)
            .unwrap_or_else(|e| panic!("SCSS 编译失败 {}: {}", file, e));
        combined.push_str(&css);
        combined.push('\n');
    }

    let katex_css_path = manifest_dir.join("assets/katex/katex-inline.min.css");
    if katex_css_path.exists() {
        let katex_css = fs::read_to_string(&katex_css_path)
            .unwrap_or_else(|_| panic!("读取 KaTeX CSS 失败"));
        combined.push_str(&katex_css);
        combined.push('\n');
    }

    let css_size = combined.len();
    fs::write(out_dir.join("global.css"), combined).unwrap();

    println!(
        "cargo:warning=compiled {} SCSS files + KaTeX CSS → global.css ({} bytes)",
        global_files.len(),
        css_size
    );

    println!("cargo:rerun-if-changed=src/styles/");
    println!("cargo:rerun-if-changed=assets/katex/katex-inline.min.css");
}
