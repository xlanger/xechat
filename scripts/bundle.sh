#!/bin/bash
# 跨平台打包脚本 - 自动检测平台并执行相应的打包

set -e

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "=== XEChat 跨平台打包工具 ==="

# 检测操作系统
case "$(uname -s)" in
    Darwin*)
        echo "检测到 macOS 系统"
        "$PROJECT_DIR/scripts/macos_bundle.sh"
        ;;
    Linux*)
        echo "检测到 Linux 系统"
        "$PROJECT_DIR/scripts/linux_bundle.sh"
        ;;
    MINGW*|MSYS*|CYGWIN*)
        echo "检测到 Windows 系统"
        echo "请使用 PowerShell 运行: powershell -ExecutionPolicy Bypass -File scripts\windows_bundle.ps1"
        exit 1
        ;;
    *)
        echo "未知系统，无法自动打包"
        exit 1
        ;;
esac
