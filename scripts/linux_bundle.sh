#!/bin/bash
# Linux 打包脚本 - 生成 AppImage 和简单的归档

set -e

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="$PROJECT_DIR/target/release"
APP_NAME="XEChat"
BUNDLE_DIR="$TARGET_DIR/$APP_NAME-linux"
APPIMAGE_DIR="$TARGET_DIR/$APP_NAME.AppDir"

echo "=== 构建 Release 版本 ==="
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml"

echo "=== 创建基础包目录 ==="
rm -rf "$BUNDLE_DIR" "$APPIMAGE_DIR"
mkdir -p "$BUNDLE_DIR" "$APPIMAGE_DIR/usr/bin" "$APPIMAGE_DIR/usr/share/icons/hicolor/256x256/apps" "$APPIMAGE_DIR/usr/share/applications"

# 复制可执行文件
cp "$TARGET_DIR/xechat" "$BUNDLE_DIR/$APP_NAME"
cp "$TARGET_DIR/xechat" "$APPIMAGE_DIR/usr/bin/$APP_NAME"
chmod +x "$BUNDLE_DIR/$APP_NAME" "$APPIMAGE_DIR/usr/bin/$APP_NAME"

# 复制图标
cp "$PROJECT_DIR/assets/icon.png" "$BUNDLE_DIR/icon.png"
cp "$PROJECT_DIR/assets/icon.png" "$APPIMAGE_DIR/usr/share/icons/hicolor/256x256/apps/$APP_NAME.png"

# 创建 .desktop 文件
cat > "$APPIMAGE_DIR/$APP_NAME.desktop" << 'DESKTOP'
[Desktop Entry]
Name=XEChat
Comment=Desktop AI Chat Client
Exec=XEChat
Icon=XEChat
Type=Application
Categories=Office;Productivity;
Terminal=false
DESKTOP

cp "$APPIMAGE_DIR/$APP_NAME.desktop" "$APPIMAGE_DIR/usr/share/applications/"

# 创建 AppRun 脚本
cat > "$APPIMAGE_DIR/AppRun" << 'APP_RUN'
#!/bin/bash
cd "$(dirname "$0")"
export APPDIR="$(pwd)"
exec "$APPDIR/usr/bin/XEChat" "$@"
APP_RUN

chmod +x "$APPIMAGE_DIR/AppRun"

# 创建 README
cat > "$BUNDLE_DIR/README.txt" << 'README'
XEChat - Desktop AI Chat Client
================================

运行方式：
- 在终端运行: ./XEChat
- 或设置为可执行后双击运行

AppImage 运行方式：
- chmod +x XEChat-*.AppImage
- ./XEChat-*.AppImage

数据路径：
- 配置文件: ~/.config/XEChat/config.toml
- 对话数据: ~/.local/share/XEChat/lancedb/
- 嵌入模型: ~/.local/share/XEChat/models/

系统要求：
- Linux (64-bit)
- glibc 2.31 或更高版本
- libwebkit2gtk-4.1-dev
README

echo ""
echo "=== 创建压缩包 ==="
cd "$TARGET_DIR"
tar -czf "$APP_NAME-linux-x86_64.tar.gz" -C "$(dirname "$BUNDLE_DIR")" "$(basename "$BUNDLE_DIR")"

echo ""
echo "=== 打包完成 ==="
echo "基础包路径: $BUNDLE_DIR"
echo "压缩包: $TARGET_DIR/$APP_NAME-linux-x86_64.tar.gz"
echo ""
echo "注意：完整的 AppImage 生成需要使用 linuxdeploy 和 appimagetool"
echo "在 Linux 系统上运行以下命令来创建 AppImage："
echo ""
echo "  1. 安装依赖："
echo "     wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
echo "     chmod +x linuxdeploy-x86_64.AppImage"
echo ""
echo "  2. 创建 AppImage："
echo "     cd $TARGET_DIR"
echo "     ARCH=x86_64 ./linuxdeploy-x86_64.AppImage --appdir $APPIMAGE_DIR --output appimage"
echo ""
