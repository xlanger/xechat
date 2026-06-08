# XEChat 打包指南

## 前置要求

- Rust 1.85+（edition 2024）
- 目标平台本地构建环境

## 快速开始

```bash
# Release 构建
cargo build --release

# 构建产物位于
ls target/release/xechat       # Linux/macOS 可执行文件
ls target/release/xechat.exe   # Windows 可执行文件
```

## 平台详细说明

### macOS

**构建**：

```bash
cargo build --release
```

**Dioxus.toml 配置**：

```toml
[application]
name = "XEChat"

[bundle]
identifier = "com.xechat"
publisher = "xlanger"
icon = ["assets/icon.png"]
resources = ["assets/"]
```

**特性**：
- 透明标题栏（`with_titlebar_transparent(true)`）
- 全尺寸内容视图（`with_fullsize_content_view(true)`）
- 自定义窗口图标
- 最小窗口尺寸 1300×680

**打包为 .app**：

```bash
# 使用 Dioxus bundle 命令
dx bundle --release

# 或手动创建 .app 结构
mkdir -p XEChat.app/Contents/MacOS
cp target/release/xechat XEChat.app/Contents/MacOS/
# 创建 Info.plist 等
```

### Linux

**依赖安装**：

```bash
# Debian/Ubuntu
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev

# Fedora
sudo dnf install webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel
```

**构建**：

```bash
cargo build --release
```

**打包为 AppImage**：

```bash
# 下载 linuxdeploy
wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

# 创建 AppImage
cd target/release
ARCH=x86_64 ./linuxdeploy-x86_64.AppImage --appdir XEChat.AppDir --output appimage
```

### Windows

**前置要求**：
- WebView2 Runtime（Windows 10/11 已内置）

**构建**：

```powershell
cargo build --release
```

**制作安装程序**：
- Inno Setup
- NSIS
- WiX Toolset

## 构建配置

### Cargo.toml Release 优化

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

### build.rs

`build.rs` 在编译时执行：
1. 使用 `grass` crate 编译全局 SCSS → `global.css`
2. 注入 KaTeX CSS
3. 输出到 `OUT_DIR/global.css`

`main.rs` 通过 `include_str!` 将 `global.css` 注入 WebView `<head>`。

### 资源文件

```
assets/
├── icon.png          # 应用图标（用于窗口图标和打包）
└── katex/
    └── katex-inline.min.css  # KaTeX 行内渲染样式
```

## 注意事项

1. 所有打包都需要先执行 `cargo build --release`
2. 建议在目标平台本地构建，确保兼容性
3. macOS 代码签名和公证需要 Apple Developer 账号
4. Windows 可能需要代码签名证书避免 SmartScreen 警告
5. `build.rs` 编译的 SCSS 变更会自动触发重新构建（`cargo:rerun-if-changed`）
