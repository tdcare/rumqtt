# ============================================================
# rumqttd OHOS (OpenHarmony) 交叉编译脚本
# ============================================================
# 用途：将 rumqttd NAPI 模块编译为 OHOS 动态链接库（.so）和类型声明（.d.ts）
#
# 前提条件：
#   1. 已安装 OHOS NDK（DevEco Studio 5.0+），并设置环境变量 OHOS_NDK_HOME
#   2. 已通过 rustup 安装以下目标平台：
#      - aarch64-unknown-linux-ohos
#      可通过以下命令安装：
#        rustup target add aarch64-unknown-linux-ohos
#   3. 已安装 ohrs CLI：
#        cargo install ohrs
#
# 产物目录结构：
#   rumqttd/ohos-libs/
#   └── arm64-v8a/
#       ├── librumqttd_napi.so
#       └── librumqttd_napi.d.ts
#
#   rumqttd/examples/ohos-app/entry/libs/arm64-v8a/
#       ├── librumqttd_napi.so
#       └── librumqttd_napi.d.ts
# ============================================================

# 遇到任何错误立即停止执行
$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " rumqttd OHOS 交叉编译" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# ----------------------------------------------------------
# 步骤 1：检查环境
# ----------------------------------------------------------

# 检查 OHOS_NDK_HOME 环境变量是否已设置
Write-Host "[1/5] 检查 OHOS_NDK_HOME 环境变量..." -ForegroundColor Yellow
if ([string]::IsNullOrEmpty($env:OHOS_NDK_HOME)) {
    Write-Host "错误：环境变量 OHOS_NDK_HOME 未设置。" -ForegroundColor Red
    Write-Host "请设置 OHOS_NDK_HOME 指向你的 OHOS NDK 安装目录，例如：" -ForegroundColor Red
    Write-Host '  $env:OHOS_NDK_HOME = "C:\DevEcoStudio\sdk\HarmonyOS-NEXT-DB6\openharmony\native"' -ForegroundColor Gray
    exit 1
}

# 检查 OHOS_NDK_HOME 路径是否存在
if (-not (Test-Path $env:OHOS_NDK_HOME)) {
    Write-Host "错误：OHOS_NDK_HOME 指向的路径不存在：$env:OHOS_NDK_HOME" -ForegroundColor Red
    Write-Host "请确认 OHOS NDK 已正确安装（通过 DevEco Studio SDK Manager）。" -ForegroundColor Red
    exit 1
}
Write-Host "  OHOS_NDK_HOME = $env:OHOS_NDK_HOME" -ForegroundColor Green

# 检查 ohrs CLI 是否已安装
Write-Host "[2/5] 检查 ohrs CLI 是否已安装..." -ForegroundColor Yellow
try {
    $ohrsVersion = & ohrs --version 2>&1
    Write-Host "  ohrs 版本：$ohrsVersion" -ForegroundColor Green
}
catch {
    Write-Host "错误：ohrs CLI 未安装或无法执行。" -ForegroundColor Red
    Write-Host "请通过以下命令安装：" -ForegroundColor Red
    Write-Host "  cargo install ohrs" -ForegroundColor Gray
    exit 1
}

# 检查 Rust OHOS target 是否已安装
Write-Host "[3/5] 检查 Rust OHOS target 是否已安装..." -ForegroundColor Yellow
$installedTargets = & rustup target list --installed 2>&1
if ($installedTargets -notcontains "aarch64-unknown-linux-ohos") {
    Write-Host "错误：Rust target aarch64-unknown-linux-ohos 未安装。" -ForegroundColor Red
    Write-Host "请通过以下命令安装：" -ForegroundColor Red
    Write-Host "  rustup target add aarch64-unknown-linux-ohos" -ForegroundColor Gray
    exit 1
}
Write-Host "  aarch64-unknown-linux-ohos 已安装" -ForegroundColor Green

# ----------------------------------------------------------
# 步骤 2：执行编译
# ----------------------------------------------------------

Write-Host "[4/5] 开始编译 OHOS NAPI 模块..." -ForegroundColor Yellow
Write-Host ""

# 获取脚本所在目录（rumqttd/src/ohos/）
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
# NAPI crate 就在脚本同目录
$napiCrateDir = $scriptDir
# rumqttd 根目录（向上两级：src/ohos/ -> src/ -> rumqttd/）
$rumqttdDir = Split-Path -Parent (Split-Path -Parent $scriptDir)

# 检查 NAPI crate 目录是否存在
if (-not (Test-Path (Join-Path $napiCrateDir "Cargo.toml"))) {
    Write-Host "错误：NAPI crate Cargo.toml 不存在：$napiCrateDir" -ForegroundColor Red
    exit 1
}

# 保存当前目录，切换到 NAPI crate 目录
$originalDir = Get-Location
Set-Location $napiCrateDir

try {
    Write-Host "工作目录：$napiCrateDir" -ForegroundColor Cyan
    Write-Host "执行编译命令..." -ForegroundColor Cyan
    Write-Host "  ohrs build" -ForegroundColor Gray
    Write-Host ""

    ohrs build

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
# 步骤 3：复制产物
# ----------------------------------------------------------

Write-Host ""
Write-Host "[5/5] 复制构建产物..." -ForegroundColor Yellow

# ohrs build 产物目录（通常在 NAPI crate 的 dist/ 或类似位置）
$distDir = Join-Path $napiCrateDir "dist"

# 查找 .so 文件（兼容不同产物路径）
$soFile = $null
$dtsFile = $null

# 优先检查 dist 目录
if (Test-Path $distDir) {
    $soFile = Get-ChildItem -Path $distDir -Filter "*.so" -Recurse | Select-Object -First 1
    $dtsFile = Get-ChildItem -Path $distDir -Filter "*.d.ts" -Recurse | Select-Object -First 1
}

# 如果 dist 目录没有，检查 target 目录
if ($null -eq $soFile) {
    $workspaceRoot = Split-Path -Parent $rumqttdDir
    $targetDir = Join-Path $workspaceRoot "target\aarch64-unknown-linux-ohos\release"
    if (Test-Path $targetDir) {
        $soFile = Get-ChildItem -Path $targetDir -Filter "librumqttd_napi.so" | Select-Object -First 1
    }
}

if ($null -eq $soFile) {
    Write-Host "警告：未找到 .so 产物文件，请检查编译输出。" -ForegroundColor Red
    Write-Host "  已检查目录：" -ForegroundColor Red
    Write-Host "    $distDir" -ForegroundColor Gray
    Write-Host "    $targetDir" -ForegroundColor Gray
    exit 1
}

Write-Host "  找到产物：$($soFile.FullName)" -ForegroundColor Green
if ($null -ne $dtsFile) {
    Write-Host "  找到声明：$($dtsFile.FullName)" -ForegroundColor Green
}

# 目标目录 1：rumqttd/ohos-libs/arm64-v8a/
$ohosLibsDir = Join-Path $rumqttdDir "ohos-libs\arm64-v8a"
if (-not (Test-Path $ohosLibsDir)) {
    New-Item -ItemType Directory -Path $ohosLibsDir -Force | Out-Null
}
Copy-Item -Path $soFile.FullName -Destination $ohosLibsDir -Force
if ($null -ne $dtsFile) {
    Copy-Item -Path $dtsFile.FullName -Destination $ohosLibsDir -Force
}
Write-Host "  已复制到：$ohosLibsDir" -ForegroundColor Green

# 目标目录 2：rumqttd/examples/ohos-app/entry/libs/arm64-v8a/
$appLibsDir = Join-Path $rumqttdDir "examples\ohos-app\entry\libs\arm64-v8a"
if (-not (Test-Path $appLibsDir)) {
    New-Item -ItemType Directory -Path $appLibsDir -Force | Out-Null
}
Copy-Item -Path $soFile.FullName -Destination $appLibsDir -Force
if ($null -ne $dtsFile) {
    Copy-Item -Path $dtsFile.FullName -Destination $appLibsDir -Force
}
Write-Host "  已复制到：$appLibsDir" -ForegroundColor Green

# ----------------------------------------------------------
# 构建结果摘要
# ----------------------------------------------------------

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " 编译成功！" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""

Write-Host "产物摘要：" -ForegroundColor Cyan

# 列出 ohos-libs 中的文件
Write-Host ""
Write-Host "  ohos-libs/arm64-v8a/" -ForegroundColor Cyan
$libFiles = Get-ChildItem -Path $ohosLibsDir -File
foreach ($file in $libFiles) {
    $sizeMB = [math]::Round($file.Length / 1MB, 2)
    Write-Host "    $($file.Name)  -  $sizeMB MB ($($file.Length) bytes)" -ForegroundColor Green
}

# 列出 ohos-app 中的文件
Write-Host ""
Write-Host "  examples/ohos-app/entry/libs/arm64-v8a/" -ForegroundColor Cyan
$appFiles = Get-ChildItem -Path $appLibsDir -File
foreach ($file in $appFiles) {
    $sizeMB = [math]::Round($file.Length / 1MB, 2)
    Write-Host "    $($file.Name)  -  $sizeMB MB ($($file.Length) bytes)" -ForegroundColor Green
}

Write-Host ""
Write-Host "完成！" -ForegroundColor Cyan
Write-Host "  .so 产物位于：$ohosLibsDir" -ForegroundColor Cyan
Write-Host "  App 产物位于：$appLibsDir" -ForegroundColor Cyan
