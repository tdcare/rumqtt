# rumqttd Android 演示应用

这是 [rumqttd](https://github.com/bytebeamio/rumqtt) MQTT Broker 的 Android 演示应用，展示如何在 Android 设备上嵌入运行一个完整的 MQTT Broker。

## 功能

- 开启/关闭 MQTT Broker（监听 `0.0.0.0:1883`）
- 实时查看运行指标（连接数、订阅数、发布消息数、失败数）
- 查看当前客户端连接列表及其详细信息

## 前置条件

- Android Studio（推荐 Hedgehog 2023.1+ 或更新版本）
- Android NDK（通过 SDK Manager 安装）
- CMake 3.22.1+（通过 SDK Manager 安装）
- 编译好的 `librumqttd.so` 共享库

## 使用步骤

### 1. 编译 librumqttd.so

在项目根目录下使用交叉编译脚本：

```powershell
cd rumqttd
.\build_android.ps1
```

编译完成后，`.so` 文件会生成在 `rumqttd/android-libs/` 目录下。

### 2. 复制 .so 文件到 jniLibs

将编译好的 `.so` 文件复制到本项目的 `app/src/main/jniLibs/` 对应 ABI 目录：

```
app/src/main/jniLibs/
├── arm64-v8a/librumqttd.so
├── armeabi-v7a/librumqttd.so
├── x86_64/librumqttd.so      # 模拟器需要
└── x86/librumqttd.so          # 模拟器需要
```

### 3. 用 Android Studio 打开项目

直接用 Android Studio 打开 `rumqttd/examples/android-app/` 目录即可。

### 4. 编译并运行

连接 Android 设备或启动模拟器，点击 Run 即可。

## 架构说明

```
Kotlin (BrokerActivity)
    ↓ 调用
Kotlin (RumqttdBridge) — JNI 封装单例
    ↓ native 方法
C (RumqttdJNI.c) — JNI 胶水层
    ↓ 链接
Rust (librumqttd.so) — MQTT Broker 核心
```

## 注意事项

- MQTT 默认使用明文传输（端口 1883），`AndroidManifest.xml` 已配置 `usesCleartextTraffic=true`
- 所有 FFI 调用均在子线程执行，不会阻塞主线程
- App 退出时会自动调用 `stop()` 和 `free()` 释放 Broker 资源
- 生产环境建议启用 TLS 加密
