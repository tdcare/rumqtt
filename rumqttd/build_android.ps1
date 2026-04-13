# ============================================================
# rumqttd Android 交叉编译脚本
# ============================================================
# 用途：将 rumqttd 编译为 Android 动态链接库（.so）和静态库（.a），支持四种 ABI 架构
#
# 前提条件：
#   1. 已安装 Android NDK，并设置环境变量 ANDROID_NDK_HOME
#   2. 已通过 rustup 安装以下目标平台：
#      - aarch64-linux-android
#      - armv7-linux-androideabi
#      - x86_64-linux-android
#      - i686-linux-android
#      可通过以下命令安装：
#        rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
#   3. 已安装 cargo-ndk：
#        cargo install cargo-ndk
#
# 产物目录结构：
#   rumqttd/android-libs/
#   ├── arm64-v8a/librumqttd.so
#   ├── armeabi-v7a/librumqttd.so
#   ├── x86_64/librumqttd.so
#   └── x86/librumqttd.so
#
#   rumqttd/examples/android-app/app/src/main/cpp/libs/
#   ├── arm64-v8a/librumqttd.a
#   ├── armeabi-v7a/librumqttd.a
#   ├── x86_64/librumqttd.a
#   └── x86/librumqttd.a
# ============================================================

# 遇到任何错误立即停止执行
$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " rumqttd Android 交叉编译" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ----------------------------------------------------------
# 步骤 1：检查环境
# ----------------------------------------------------------

# 检查 ANDROID_NDK_HOME 环境变量是否已设置
Write-Host "[1/4] 检查 ANDROID_NDK_HOME 环境变量..." -ForegroundColor Yellow
if ([string]::IsNullOrEmpty($env:ANDROID_NDK_HOME)) {
    Write-Host "错误：环境变量 ANDROID_NDK_HOME 未设置。" -ForegroundColor Red
    Write-Host "请设置 ANDROID_NDK_HOME 指向你的 Android NDK 安装目录，例如：" -ForegroundColor Red
    Write-Host '  $env:ANDROID_NDK_HOME = "C:\Android\ndk\25.2.9519653"' -ForegroundColor Gray
    exit 1
}

# 检查 ANDROID_NDK_HOME 路径是否存在
if (-not (Test-Path $env:ANDROID_NDK_HOME)) {
    Write-Host "错误：ANDROID_NDK_HOME 指向的路径不存在：$env:ANDROID_NDK_HOME" -ForegroundColor Red
    Write-Host "请确认 Android NDK 已正确安装。" -ForegroundColor Red
    exit 1
}
Write-Host "  ANDROID_NDK_HOME = $env:ANDROID_NDK_HOME" -ForegroundColor Green

# 检查 cargo-ndk 是否已安装
Write-Host "[2/4] 检查 cargo-ndk 是否已安装..." -ForegroundColor Yellow
try {
    $ndkVersion = & cargo ndk --version 2>&1
    Write-Host "  cargo-ndk 版本：$ndkVersion" -ForegroundColor Green
}
catch {
    Write-Host "错误：cargo-ndk 未安装或无法执行。" -ForegroundColor Red
    Write-Host "请通过以下命令安装：" -ForegroundColor Red
    Write-Host "  cargo install cargo-ndk" -ForegroundColor Gray
    exit 1
}

# ----------------------------------------------------------
# 步骤 2：切换到 workspace 根目录并执行编译
# ----------------------------------------------------------

Write-Host "[3/4] 开始编译四种架构（arm64-v8a, armeabi-v7a, x86_64, x86）..." -ForegroundColor Yellow
Write-Host ""

# 获取脚本所在目录的上级目录（workspace 根目录）
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$workspaceRoot = Split-Path -Parent $scriptDir

# 保存当前目录，切换到 workspace 根目录执行编译
$originalDir = Get-Location
Set-Location $workspaceRoot

try {
    # 使用 cargo ndk 编译所有四种架构
    # -t 指定目标 ABI
    # -o 指定输出目录（cargo-ndk 会自动创建 ABI 子目录）
    # --release 使用 release 模式编译
    # -p rumqttd 仅编译 rumqttd 包
    # --lib 仅编译库目标
    # --no-default-features 禁用默认 features
    # --features use-rustls,websocket 启用 TLS 和 WebSocket 支持
    Write-Host "执行编译命令..." -ForegroundColor Cyan
    Write-Host "  cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -t x86 -o ./rumqttd/android-libs build --release -p rumqttd --lib --no-default-features --features use-rustls,websocket" -ForegroundColor Gray
    Write-Host ""

    cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -t x86 -o ./rumqttd/android-libs build --release -p rumqttd --lib --no-default-features --features "use-rustls,websocket"

    if ($LASTEXITCODE -ne 0) {
        Write-Host "错误：编译失败，退出码：$LASTEXITCODE" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}
finally {
    # 无论成功与否，恢复原始工作目录
    Set-Location $originalDir
}

# ----------------------------------------------------------
# 步骤 3：输出编译结果
# ----------------------------------------------------------

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " 编译成功！" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""

# 列出所有生成的 .so 文件及其大小
$outputDir = Join-Path $workspaceRoot "rumqttd\android-libs"
Write-Host "产物目录：$outputDir" -ForegroundColor Cyan
Write-Host ""

$abis = @("arm64-v8a", "armeabi-v7a", "x86_64", "x86")
foreach ($abi in $abis) {
    $soFile = Join-Path $outputDir "$abi\librumqttd.so"
    if (Test-Path $soFile) {
        $fileInfo = Get-Item $soFile
        $sizeMB = [math]::Round($fileInfo.Length / 1MB, 2)
        Write-Host "  $abi/librumqttd.so  -  $sizeMB MB ($($fileInfo.Length) bytes)" -ForegroundColor Green
    }
    else {
        Write-Host "  $abi/librumqttd.so  -  未找到！" -ForegroundColor Red
    }
}

# ----------------------------------------------------------
# 步骤 4：复制静态库（.a）到 Android 项目的 cpp/libs 目录
# ----------------------------------------------------------

Write-Host ""
Write-Host "[4/4] 复制静态库（.a）到 Android 项目..." -ForegroundColor Yellow

$cppLibsDir = Join-Path $workspaceRoot "rumqttd\examples\android-app\app\src\main\cpp\libs"

# Rust target 到 Android ABI 的映射
$archMap = @{
    "aarch64-linux-android"    = "arm64-v8a"
    "armv7-linux-androideabi"  = "armeabi-v7a"
    "x86_64-linux-android"     = "x86_64"
    "i686-linux-android"       = "x86"
}

foreach ($entry in $archMap.GetEnumerator()) {
    $rustTarget = $entry.Key
    $abi = $entry.Value
    $srcFile = Join-Path $workspaceRoot "target\$rustTarget\release\librumqttd.a"
    $dstDir = Join-Path $cppLibsDir $abi
    $dstFile = Join-Path $dstDir "librumqttd.a"

    if (Test-Path $srcFile) {
        if (-not (Test-Path $dstDir)) {
            New-Item -ItemType Directory -Path $dstDir -Force | Out-Null
        }
        Copy-Item -Path $srcFile -Destination $dstFile -Force
        $fileInfo = Get-Item $dstFile
        $sizeMB = [math]::Round($fileInfo.Length / 1MB, 2)
        Write-Host "  $abi/librumqttd.a  -  $sizeMB MB" -ForegroundColor Green
    }
    else {
        Write-Host "  $abi/librumqttd.a  -  源文件未找到：$srcFile" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "完成！" -ForegroundColor Cyan
Write-Host "  .so 产物位于：$outputDir" -ForegroundColor Cyan
Write-Host "  .a  产物位于：$cppLibsDir" -ForegroundColor Cyan
