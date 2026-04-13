# rumqttd Android 集成示例

本目录提供了在 Android 应用中集成 rumqttd MQTT Broker 共享库的完整示例代码。

## 文件说明

| 文件 | 说明 |
|------|------|
| `RumqttdBridge.kt` | Kotlin JNI 封装单例，加载 `librumqttd.so` 并封装所有 native 方法 |
| `RumqttdJNI.c` | C 语言 JNI 胶水层，桥接 Java native 方法到 rumqttd FFI 函数 |
| `BrokerModels.kt` | Kotlin 数据类，用于解析 FFI 返回的 JSON 数据 |
| `BrokerActivity.kt` | 主界面 Activity，提供 Broker 控制和状态监控 UI |
| `activity_broker.xml` | Activity 布局文件（Material Design 风格） |
| `item_connection.xml` | 客户端连接列表项布局 |

## 集成步骤

### 1. 放置共享库

将交叉编译好的 `librumqttd.so` 放到对应 ABI 目录下：

```
app/
└── src/
    └── main/
        └── jniLibs/
            ├── arm64-v8a/
            │   └── librumqttd.so
            ├── armeabi-v7a/
            │   └── librumqttd.so
            └── x86_64/          # 仅模拟器需要
                └── librumqttd.so
```

### 2. Gradle 配置

在 `app/build.gradle` 中确认以下配置：

```groovy
android {
    defaultConfig {
        ndk {
            // 按需选择目标 ABI，通常只需 arm64-v8a
            abiFilters 'arm64-v8a', 'armeabi-v7a'
        }
    }

    // 如果需要编译 JNI C 代码（RumqttdJNI.c），添加 externalNativeBuild
    // 如果直接使用预编译 .so，则不需要
}

dependencies {
    // 用于 JSON 解析
    implementation 'com.google.code.gson:gson:2.10.1'
    // Material Design 组件
    implementation 'com.google.android.material:material:1.11.0'
}
```

### 3. AndroidManifest.xml 权限

MQTT Broker 需要网络权限来监听端口和接受客户端连接：

```xml
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <!-- 必需：MQTT Broker 监听端口需要网络权限 -->
    <uses-permission android:name="android.permission.INTERNET" />
    <!-- 可选：获取网络状态信息 -->
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
</manifest>
```

### 4. 复制源代码

- 将 `RumqttdBridge.kt`、`BrokerModels.kt`、`BrokerActivity.kt` 复制到 `app/src/main/java/com/example/rumqttd/` 目录
- 将布局文件复制到 `app/src/main/res/layout/` 目录
- 将 `RumqttdJNI.c` 和 `rumqttd.h` 放到 `app/src/main/jni/` 目录（如需本地编译 JNI）

### 5. 编译 JNI 胶水层

有两种方式：

**方式 A：预编译（推荐）**

使用 NDK 交叉编译 `RumqttdJNI.c`，将其与 `librumqttd.so` 链接后生成最终的 `.so` 文件。

**方式 B：CMake 集成**

在 `app/src/main/jni/CMakeLists.txt` 中：

```cmake
cmake_minimum_required(VERSION 3.10)

add_library(rumqttd_jni SHARED RumqttdJNI.c)

# 链接预编译的 librumqttd.so
add_library(rumqttd SHARED IMPORTED)
set_target_properties(rumqttd PROPERTIES
    IMPORTED_LOCATION ${CMAKE_SOURCE_DIR}/../jniLibs/${ANDROID_ABI}/librumqttd.so)

target_link_libraries(rumqttd_jni rumqttd)
```

> **注意**：如果将 JNI 代码直接编译进 `librumqttd.so`（在 Rust 侧通过 `#[no_mangle]` 导出 JNI 函数），则不需要单独的 JNI 编译步骤。

## 默认配置

示例默认监听 `0.0.0.0:1883`（MQTT v4），不启用 TLS 和 WebSocket，适合开发测试。

## 注意事项

- Android 不允许在主线程进行网络操作，所有 FFI 调用应在子线程执行
- Broker 端口号不要与设备上其他服务冲突
- 生产环境建议启用 TLS 加密
- 确保 App 退出时正确调用 `stop()` 和 `free()` 释放资源
