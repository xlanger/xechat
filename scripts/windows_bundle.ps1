# Windows 打包脚本
# 使用 PowerShell 运行
# PowerShell -ExecutionPolicy Bypass -File scripts\windows_bundle.ps1

$ErrorActionPreference = "Stop"

# 获取项目根目录
$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ProjectDir

$TargetDir = Join-Path $ProjectDir "target\release"
$AppName = "XEChat"
$BundleDir = Join-Path $TargetDir $AppName

Write-Host "=== 构建 Release 版本 ==="
cargo build --release --manifest-path (Join-Path $ProjectDir "Cargo.toml")

Write-Host "=== 创建 Windows 包目录 ==="
Remove-Item -Path $BundleDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -Path $BundleDir -ItemType Directory -Force | Out-Null

# 复制可执行文件
Copy-Item (Join-Path $TargetDir "xechat.exe") (Join-Path $BundleDir "$AppName.exe")

# 复制图标
$IconPath = Join-Path $ProjectDir "assets\icons\icon.ico"
if (Test-Path $IconPath) {
    Copy-Item $IconPath (Join-Path $BundleDir "icon.ico")
}

# 创建 README
@"
XEChat - Desktop AI Chat Client
================================

运行方式：
- 双击 $AppName.exe 运行

数据路径：
- 配置文件: %APPDATA%\XEChat\config.toml
- 对话数据: %LOCALAPPDATA%\XEChat\lancedb\
- 嵌入模型: %LOCALAPPDATA%\XEChat\models\

系统要求：
- Windows 10 或更高版本
- WebView2 Runtime
"@ | Out-File -FilePath (Join-Path $BundleDir "README.txt") -Encoding UTF8

Write-Host ""
Write-Host "=== 打包完成 ==="
Write-Host "包路径: $BundleDir"
Write-Host ""
Write-Host "下一步："
Write-Host "1. 可以直接分发 $BundleDir 目录"
Write-Host "2. 或者使用 Inno Setup/NSIS 制作安装程序"
