#!/bin/bash
set -e

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${TARGET_DIR:-$PROJECT_DIR/target/release}"
APP_NAME="XEChat"
BUNDLE_DIR="$TARGET_DIR/$APP_NAME.app"

echo "=== 创建 macOS .app Bundle ==="
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR"/Contents/{MacOS,Resources}

cp "$TARGET_DIR/xechat" "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"

# 图标路径
if [ -f "$PROJECT_DIR/assets/icons/icon.icns" ]; then
    cp "$PROJECT_DIR/assets/icons/icon.icns" "$BUNDLE_DIR/Contents/Resources/icon.icns"
elif [ -f "$PROJECT_DIR/assets/icon.png" ]; then
    cp "$PROJECT_DIR/assets/icon.png" "$BUNDLE_DIR/Contents/Resources/icon.png"
fi

cat > "$BUNDLE_DIR/Contents/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>zh_CN</string>
    <key>CFBundleDisplayName</key>
    <string>XEChat</string>
    <key>CFBundleExecutable</key>
    <string>XEChat</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>com.xechat.app</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>XEChat</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
PLIST

echo "=== Bundle 创建完成 ==="
echo "App 路径: $BUNDLE_DIR"
echo ""
echo "运行方式: open \"$BUNDLE_DIR\""
echo "或直接双击 Finder 中的 XEChat.app"
echo ""
echo "配置文件路径: ~/Library/Application Support/XEChat/config.toml"
echo "数据路径: ~/Library/Application Support/XEChat/lancedb/"
