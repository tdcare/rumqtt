# rumqttd OHOS 演示应用

这是 [rumqttd](https://github.com/bytebeamio/rumqtt) MQTT Broker 的 OpenHarmony / HarmonyOS NEXT 演示应用，展示如何在 OHOS 设备上嵌入运行一个完整的 MQTT Broker。

## 功能

- 开启/关闭 MQTT Broker（监听 `0.0.0.0:1883`）
- 实时查看运行指标（连接数、订阅数、发布消息数、失败数）
- 查看当前客户端连接列表及其详细信息

## 前置条件

- DevEco Studio 5.0+（HarmonyOS NEXT 开发环境）
- HarmonyOS NEXT SDK（API 12+）
- 编译好的 `librumqttd_napi.so` 原生库

## 使用步骤

### 1. 编译 librumqttd_napi.so

在项目根目录下使用交叉编译脚本：

```powershell
cd rumqttd/src/ohos
.\build_ohos.ps1
```

编译完成后，`.so` 文件会自动复制到 `rumqttd/examples/ohos-app/entry/libs/arm64-v8a/` 目录。

如需手动编译，请参考 [rumqttd/examples/ohos/README.md](../ohos/README.md) 中的构建说明。

### 2. 确认 .so 文件位置

确保 `.so` 文件已放置在正确目录：

```
entry/
└── libs/
    └── arm64-v8a/
        ├── librumqttd_napi.so
        └── librumqttd_napi.d.ts   # 可选
```

### 3. 用 DevEco Studio 打开项目

直接用 DevEco Studio 打开 `rumqttd/examples/ohos-app/` 目录即可。首次打开会自动同步项目依赖。

### 4. 编译并运行

连接 OHOS 真机设备或启动模拟器，点击 Run 即可。

> **提示**：如使用模拟器，请确认模拟器架构与编译的 `.so` 架构一致（默认为 arm64-v8a）。

## 架构说明

```
ArkTS (BrokerPage.ets) — UI 界面
    ↓ 调用
ArkTS (RumqttdBridge.ets) — NAPI 封装类
    ↓ import from 'librumqttd_napi.so'
Rust (librumqttd_napi.so) — MQTT Broker 核心（ohos-rs NAPI 模块）
```

## 项目结构

```
ohos-app/
├── entry/
│   ├── libs/
│   │   └── arm64-v8a/          # 原生库放置目录
│   │       └── librumqttd_napi.so
│   └── src/
│       └── main/
│           ├── ets/            # ArkTS 源代码
│           └── module.json5    # 模块配置（含权限声明）
├── build-profile.json5
├── hvigorfile.ts
└── oh-package.json5
```

## 注意事项

- 需要在 `module.json5` 中声明 `ohos.permission.INTERNET` 网络权限
- MQTT 默认使用明文传输（端口 1883），生产环境建议启用 TLS
- 所有 NAPI 调用在子线程执行，不会阻塞主线程
- App 退出时会自动调用 `stop()` 和 `free()` 释放 Broker 资源
- 端口 1883 不要与设备上其他服务冲突
- 当前仅支持 arm64-v8a 架构
