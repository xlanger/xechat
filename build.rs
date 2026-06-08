use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let styles_dir = manifest_dir.join("src/styles");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let global_files = [
        "theme.scss",
        "materials.scss",
        "reset.scss",
        "keyframes.scss",
        "utilities.scss",
        "markdown.scss",
    ];

    let mut combined = String::new();
    let options = grass::Options::default().load_path(&styles_dir);

    for file in &global_files {
        let path = styles_dir.join(file);
        if path.exists() {
            let content = fs::read_to_string(&path).unwrap_or_else(|_| panic!("读取失败: {}", file));
            let css = grass::from_string(content, &options)
                .unwrap_or_else(|e| panic!("SCSS 编译失败 {}: {}", file, e));
            combined.push_str(&css);
            combined.push('\n');
        }
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

    println!("cargo:warning=compiled {} SCSS files + KaTeX CSS → global.css ({} bytes)",
        global_files.len(), css_size);

    println!("cargo:rerun-if-changed=src/styles/theme.scss");
    println!("cargo:rerun-if-changed=src/styles/materials.scss");
    println!("cargo:rerun-if-changed=src/styles/reset.scss");
    println!("cargo:rerun-if-changed=src/styles/keyframes.scss");
    println!("cargo:rerun-if-changed=src/styles/utilities.scss");
    println!("cargo:rerun-if-changed=src/styles/markdown.scss");
    println!("cargo:rerun-if-changed=assets/katex/katex-inline.min.css");
}
