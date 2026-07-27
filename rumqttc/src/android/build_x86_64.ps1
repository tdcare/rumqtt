# ============================================================
# rumqttc-android + rumqttd x86_64 交叉编译
# ============================================================
$ErrorActionPreference = "Stop"

$ndk = $env:ANDROID_NDK_HOME
if ([string]::IsNullOrEmpty($ndk)) {
    $ndk = "$env:LOCALAPPDATA\Android\Sdk\ndk\25.1.8937393"
}
$tc = "$ndk\toolchains\llvm\prebuilt\windows-x86_64\bin"

$env:ANDROID_NDK_HOME = $ndk
$env:PATH = "$tc;$env:PATH"
$env:CC_x86_64_linux_android  = "$tc\x86_64-linux-android21-clang.cmd"
$env:CXX_x86_64_linux_android = "$tc\x86_64-linux-android21-clang++.cmd"
$env:AR_x86_64_linux_android  = "$tc\llvm-ar.exe"
$env:CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER = "$tc\x86_64-linux-android21-clang.cmd"

# 脚本位于 rumqttc/src/android/，需要上溯 3 级到 rumqtt/ workspace root
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$workspaceRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $scriptDir))

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " rumqttc-android x86_64 编译" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  workspace: $workspaceRoot" -ForegroundColor Gray

Set-Location $workspaceRoot

cargo build --release -p rumqttc-android --target x86_64-linux-android --manifest-path rumqttc\src\android\Cargo.toml
if ($LASTEXITCODE -ne 0) {
    Write-Host "rumqttc-android 编译失败" -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "rumqttc-android x86_64 编译成功 ✓" -ForegroundColor Green

# 收集 rumqttc 产物
$outDir = Join-Path $workspaceRoot "rumqttc\android-libs\x86_64"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$soSrc = Join-Path $workspaceRoot "target\x86_64-linux-android\release\librumqttc_android.so"
if (Test-Path $soSrc) {
    Copy-Item $soSrc "$outDir\librumqttc_android.so" -Force
    Write-Host "  复制: $soSrc -> $outDir" -ForegroundColor Gray
}

# 部署到 jniLibs
$jniDir = Join-Path $workspaceRoot "..\android\smartward-rust-bridge\src\main\jniLibs\x86_64"
$jniDir = (Resolve-Path $jniDir -ErrorAction SilentlyContinue).Path
if (-not $jniDir) {
    # fallback: 手动构造绝对路径
    $jniDir = Join-Path (Split-Path -Parent $workspaceRoot) "android\smartward-rust-bridge\src\main\jniLibs\x86_64"
}
New-Item -ItemType Directory -Force -Path $jniDir | Out-Null

if (Test-Path "$outDir\librumqttc_android.so") {
    Copy-Item "$outDir\librumqttc_android.so" $jniDir -Force
    Write-Host "  部署到: $jniDir" -ForegroundColor Gray
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " rumqttd x86_64 编译" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

cargo build --release -p rumqttd --target x86_64-linux-android --lib --no-default-features --features "use-rustls,websocket"
if ($LASTEXITCODE -ne 0) {
    Write-Host "rumqttd 编译失败" -ForegroundColor Red
    exit $LASTEXITCODE
}
Write-Host "rumqttd x86_64 编译成功 ✓" -ForegroundColor Green

# 收集 rumqttd 产物
$outDir2 = Join-Path $workspaceRoot "rumqttd\android-libs\x86_64"
New-Item -ItemType Directory -Force -Path $outDir2 | Out-Null

$soSrc2 = Join-Path $workspaceRoot "target\x86_64-linux-android\release\librumqttd.so"
if (Test-Path $soSrc2) {
    Copy-Item $soSrc2 "$outDir2\librumqttd.so" -Force
    Write-Host "  复制: $soSrc2 -> $outDir2" -ForegroundColor Gray
}

if (Test-Path "$outDir2\librumqttd.so") {
    Copy-Item "$outDir2\librumqttd.so" $jniDir -Force
    Write-Host "  部署到: $jniDir" -ForegroundColor Gray
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host " 全部完成！" -ForegroundColor Green
Get-ChildItem $jniDir -File | ForEach-Object {
    $sizeMB = [math]::Round($_.Length / 1MB, 2)
    Write-Host "  $($_.Name) - $sizeMB MB" -ForegroundColor Cyan
}
Write-Host "========================================" -ForegroundColor Cyan
