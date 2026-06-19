# rumqttc Android 集成示例

本目录提供了在 Android 应用中集成 rumqttc MQTT 客户端库的完整示例代码。

## 架构概述

```
rumqttc/
├── src/android/                  ← Rust FFI 层（编译为静态库）
│   ├── src/lib.rs                   Rust FFI 导出函数
│   ├── include/rumqttc.h            C 头文件
│   ├── Cargo.toml                   Rust 项目配置
│   └── build_android.ps1            交叉编译脚本
│
└── examples/android/             ← 应用层代码（本目录）
    ├── jni/
    │   ├── RumqttcJNI.c             C JNI 胶水层（桥接 Java ↔ C FFI）
    │   └── CMakeLists.txt           CMake 构建配置
    ├── kotlin/
    │   ├── RumqttcBridge.kt         Kotlin JNI 封装单例
    │   ├── MqttModels.kt            数据模型（解析 JSON 事件）
    │   └── MqttClientActivity.kt    完整可运行的 Activity
    ├── res/
    │   └── activity_mqtt_client.xml  布局文件
    └── README.md                    本文件
```

## 文件说明

| 文件 | 说明 |
|------|------|
| `jni/RumqttcJNI.c` | C 语言 JNI 胶水层，桥接 Java native 方法到 rumqttc FFI 函数 |
| `jni/CMakeLists.txt` | CMake 构建配置，链接 Rust 静态库 |
| `kotlin/RumqttcBridge.kt` | Kotlin JNI 封装单例，加载 `librumqttc_jni.so` 并封装所有 native 方法 |
| `kotlin/MqttModels.kt` | Kotlin 数据类，用于解析 FFI 返回的 JSON 事件 |
| `kotlin/MqttClientActivity.kt` | 主界面 Activity，提供连接/订阅/发布/消息展示功能 |
| `res/activity_mqtt_client.xml` | Activity 布局文件（Material Design 风格） |

## 环境要求

- **Android Studio** Arctic Fox 或更高版本
- **Android NDK** r21 或更高版本
- **Rust 工具链** + `cargo-ndk`（用于交叉编译 Rust 代码）
- **目标 API**: Android 6.0 (API 23) 及以上

安装 `cargo-ndk`：

```bash
cargo install cargo-ndk
rustup target add aarch64-linux-android armv7-linux-androideabi
```

## 编译步骤

### 1. 编译 Rust 静态库

在项目根目录执行交叉编译脚本：

```powershell
cd rumqttc/src/android
.\build_android.ps1
```

脚本会为各目标架构生成 `librumqttc_android.a` 静态库文件。

### 2. 放置静态库

将编译好的静态库放到 JNI 构建目录：

```
rumqttc/examples/android/jni/libs/
├── arm64-v8a/
│   └── librumqttc_android.a
├── armeabi-v7a/
│   └── librumqttc_android.a
└── x86_64/              # 仅模拟器需要
    └── librumqttc_android.a
```

### 3. Android Studio 集成

在 `app/build.gradle` 中配置：

```groovy
android {
    defaultConfig {
        ndk {
            abiFilters 'arm64-v8a', 'armeabi-v7a'
        }
        externalNativeBuild {
            cmake {
                cppFlags ""
            }
        }
    }
    externalNativeBuild {
        cmake {
            path "src/main/jni/CMakeLists.txt"
            version "3.22.1"
        }
    }
}

dependencies {
    // 用于 JSON 解析
    implementation 'com.google.code.gson:gson:2.10.1'
}
```

### 4. AndroidManifest.xml 权限

MQTT 客户端需要网络权限来连接 Broker：

```xml
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <!-- 必需：连接 MQTT Broker 需要网络权限 -->
    <uses-permission android:name="android.permission.INTERNET" />
    <!-- 可选：获取网络状态信息 -->
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
</manifest>
```

### 5. 复制源代码

- 将 `kotlin/*.kt` 复制到 `app/src/main/java/com/example/rumqttc/` 目录
- 将 `res/activity_mqtt_client.xml` 复制到 `app/src/main/res/layout/` 目录
- 将 `jni/RumqttcJNI.c` 和 `jni/CMakeLists.txt` 复制到 `app/src/main/jni/` 目录
- 将 `rumqttc.h` 头文件（位于 `rumqttc/src/android/include/`）复制到 JNI 可访问的路径

## API 说明

### RumqttcBridge 单例

| 方法 | 说明 |
|------|------|
| `create(brokerUrl, clientId, ...)` | 创建客户端并自动连接，返回 Boolean |
| `free()` | 释放客户端资源 |
| `subscribe(topic, qos)` | 订阅主题 |
| `unsubscribe(topic)` | 取消订阅 |
| `publish(topic, payload, qos, retain)` | 发布消息 |
| `disconnect()` | 断开连接 |
| `isConnected()` | 检查是否已连接 |
| `pollEvent()` | 轮询单个事件（JSON 对象） |
| `pollAllEvents()` | 轮询所有待处理事件（JSON 数组） |
| `getLastError()` | 获取最近一次错误信息 |

### 事件格式

事件以 JSON 对象返回，`type` 字段标识类型：

```json
{"type": "connected"}
{"type": "disconnected"}
{"type": "message", "topic": "test/hello", "payload": "world", "qos": 1, "retain": false}
{"type": "error", "error": "Connection refused"}
```

## 注意事项

- **网络权限**：必须在 `AndroidManifest.xml` 中声明 `INTERNET` 权限
- **后台线程**：Android 不允许在主线程进行网络操作，所有 FFI 调用应在子线程执行（示例中使用 `Executors.newSingleThreadExecutor()`）
- **事件轮询**：示例使用 `Handler.postDelayed` 每 200ms 轮询事件，可根据需要调整间隔
- **内存管理**：`RumqttcBridge.free()` 必须在不再使用客户端时调用，示例在 `onDestroy()` 中自动处理
- **自动重连**：客户端内置自动重连机制，断线后会自动尝试重新连接
- **静态链接**：Rust 库以静态库（`.a`）形式链接到 JNI 动态库中，最终只需分发一个 `librumqttc_jni.so`
