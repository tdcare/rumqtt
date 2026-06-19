# rumqttd OHOS 集成示例

本目录提供了在 OpenHarmony / HarmonyOS NEXT 应用中集成 rumqttd MQTT Broker 的完整示例代码。通过 [ohos-rs (napi-ohos)](https://ohos-rs.aspect.im/) 框架，将 Rust 实现的 MQTT Broker 以 NAPI 原生模块的形式暴露给 ArkTS 层。

## 架构

```
ArkTS UI (BrokerPage.ets)
    ↓ 状态管理 + 事件回调
ArkTS Bridge (RumqttdBridge.ets) — NAPI 封装类
    ↓ import { RumqttdBroker } from 'librumqttd_napi.so'
Rust NAPI Module (rumqttd-napi) — ohos-rs 生成的原生模块
    ↓ 调用
rumqttd Broker Core — Rust MQTT Broker 引擎
```

与 Android 集成方案（C FFI + JNI 胶水层）不同，OHOS 方案使用 ohos-rs 直接生成 NAPI 接口，无需中间 C 层，架构更简洁。

## 文件说明

| 文件 | 说明 |
|------|------|
| `RumqttdBridge.ets` | ArkTS 封装类，加载 `librumqttd_napi.so` 并封装所有 NAPI 方法 |
| `BrokerModels.ets` | ArkTS 数据模型，用于解析 NAPI 返回的 JSON 数据（连接信息、路由器指标、告警） |
| `BrokerPage.ets` | 主界面组件（`@Entry @Component`），提供 Broker 控制和状态监控 UI |
| `../../src/ohos/` | Rust NAPI crate，使用 ohos-rs 将 rumqttd 功能暴露为 NAPI 接口（位于 `rumqttd/src/ohos/`） |
| `../../src/ohos/src/lib.rs` | NAPI 模块实现，定义 `RumqttdBroker` 类及其所有方法 |

## 前置条件

- **Rust**（1.70+），并安装 OHOS target：
  ```bash
  rustup target add aarch64-unknown-linux-ohos
  ```
- **ohrs CLI**（ohos-rs 构建工具）：
  ```bash
  cargo install ohrs
  ```
- **OHOS NDK**（通过 DevEco Studio 5.0+ 的 SDK Manager 安装）
- **环境变量** `OHOS_NDK_HOME` 指向 NDK 目录，例如：
  ```powershell
  $env:OHOS_NDK_HOME = "C:\DevEcoStudio\sdk\HarmonyOS-NEXT-DB6\openharmony\native"
  ```

## 构建原生库

### 使用构建脚本（推荐）

在 `rumqttd/src/ohos/` 目录下运行 PowerShell 脚本：

```powershell
cd rumqttd/src/ohos
.\build_ohos.ps1
```

脚本会自动执行以下操作：
1. 检查 `OHOS_NDK_HOME`、`ohrs`、Rust target 等环境
2. 进入 `src/ohos` 目录执行 `ohrs build`
3. 将产物（`.so` + `.d.ts`）复制到 `rumqttd/ohos-libs/arm64-v8a/` 和 `examples/ohos-app/entry/libs/arm64-v8a/`

### 手动构建

```powershell
cd rumqttd/src/ohos
ohrs build
```

构建产物包括：
- `librumqttd_napi.so` — NAPI 动态链接库
- `librumqttd_napi.d.ts` — TypeScript 类型声明（可选）

## 集成步骤

### 1. 放置原生库

将编译好的 `librumqttd_napi.so` 放到 OHOS 项目的 libs 目录下：

```
entry/
└── libs/
    └── arm64-v8a/
        ├── librumqttd_napi.so
        └── librumqttd_napi.d.ts   # 可选，提供类型提示
```

### 2. 配置 oh-package.json5

在模块的 `oh-package.json5` 中确认原生库路径配置正确：

```json5
{
  "name": "entry",
  "version": "1.0.0",
  "main": "./ets/entryability/EntryAbility.ets"
}
```

> DevEco Studio 默认会扫描 `entry/libs/` 目录下的 `.so` 文件，通常无需额外配置。

### 3. 网络权限

MQTT Broker 需要网络权限来监听端口和接受客户端连接。在 `module.json5` 中添加：

```json5
{
  "module": {
    "requestPermissions": [
      {
        "name": "ohos.permission.INTERNET"
      }
    ]
  }
}
```

### 4. 复制源代码

将以下 ArkTS 文件复制到你的项目 `ets/` 目录：

- `RumqttdBridge.ets` — NAPI 封装类
- `BrokerModels.ets` — 数据模型
- `BrokerPage.ets` — UI 界面（可按需修改或仅参考）

### 5. 导入并使用

```typescript
import { RumqttdBridge } from './RumqttdBridge';

const bridge = new RumqttdBridge();
bridge.create(configToml);
bridge.start();
```

## API 参考

### RumqttdBroker（NAPI 原生类）

从 `librumqttd_napi.so` 导入的原生类，由 Rust 侧 ohos-rs 生成。

| 方法 | 签名 | 说明 |
|------|------|------|
| `constructor` | `new RumqttdBroker(configToml: string)` | 从 TOML 配置字符串创建 Broker 实例，内部启动路由器 |
| `start` | `start(): void` | 在后台线程启动 MQTT 服务监听器（TCP/TLS/WebSocket），非阻塞 |
| `stop` | `stop(): void` | 停止 Broker，分离服务线程 |
| `getConnections` | `getConnections(): string` | 获取活跃连接列表，返回 JSON 字符串（`ConnectionInfo[]`） |
| `getMeters` | `getMeters(): string` | 获取路由器指标，返回 JSON 字符串（`Meter[]`），非阻塞 |
| `getAlerts` | `getAlerts(): string` | 获取告警信息，返回 JSON 字符串（`AlertInfo[]`），非阻塞 |
| `isRunning` | `get isRunning(): boolean` | 只读属性，MQTT 服务是否正在运行 |

### RumqttdBridge（ArkTS 封装类）

对 NAPI 原生类的二次封装，提供错误处理和类型转换。

| 方法 | 签名 | 说明 |
|------|------|------|
| `create` | `create(configToml: string): boolean` | 创建 Broker 实例 |
| `start` | `start(): boolean` | 启动 Broker |
| `stop` | `stop(): boolean` | 停止 Broker |
| `free` | `free(): void` | 释放 Broker 资源 |
| `getConnections` | `getConnections(): ConnectionInfo[]` | 获取活跃连接列表（已解析） |
| `getMeters` | `getMeters(): RouterMeter \| null` | 获取最新路由器指标（已解析） |
| `getAlerts` | `getAlerts(): AlertInfo[]` | 获取告警列表（已解析） |
| `isCreated` | `get isCreated: boolean` | Broker 实例是否已创建 |
| `isRunning` | `get isRunning: boolean` | Broker 是否正在运行 |

## 配置

Broker 通过 TOML 字符串进行配置，传入 `RumqttdBroker` 构造函数。示例配置：

```toml
id = 0

[router]
id = 0
max_connections = 1010
max_outgoing_packet_count = 200
max_segment_size = 104857600
max_segment_count = 10

# MQTT v4 监听器
[v4.1]
name = "v4-1"
listen = "0.0.0.0:1883"
next_connection_delay_ms = 1
    [v4.1.connections]
    connection_timeout_ms = 60000
    max_payload_size = 20480
    max_inflight_count = 100
    dynamic_filters = true

# 控制台（可选）
[console]
listen = "0.0.0.0:3030"
```

### 常用配置项

| 配置项 | 说明 | 默认值 |
|--------|------|--------|
| `router.max_connections` | 最大连接数 | 1010 |
| `router.max_segment_size` | 单个日志段最大字节数 | 104857600 (100MB) |
| `v4.*.listen` | MQTT v4 监听地址 | `0.0.0.0:1883` |
| `v4.*.connections.max_payload_size` | 最大消息载荷字节数 | 20480 |
| `v4.*.connections.connection_timeout_ms` | 连接超时毫秒数 | 60000 |
| `console.listen` | 管理控制台监听地址 | `0.0.0.0:3030` |

## 使用示例（ArkTS）

```typescript
import { RumqttdBridge } from './RumqttdBridge';
import { ConnectionInfo, RouterMeter } from './BrokerModels';

// 创建 bridge 实例
const bridge = new RumqttdBridge();

// TOML 配置
const config = `id = 0
[router]
id = 0
max_connections = 1010
max_outgoing_packet_count = 200
max_segment_size = 104857600
max_segment_count = 10
[v4.1]
name = "v4-1"
listen = "0.0.0.0:1883"
next_connection_delay_ms = 1
  [v4.1.connections]
  connection_timeout_ms = 60000
  max_payload_size = 20480
  max_inflight_count = 100
  dynamic_filters = true`;

// 创建并启动
bridge.create(config);
bridge.start();

// 查询连接信息
const connections: ConnectionInfo[] = bridge.getConnections();
console.log(`当前连接数: ${connections.length}`);

// 查询路由器指标
const meter: RouterMeter | null = bridge.getMeters();
if (meter !== null) {
  console.log(`总连接: ${meter.total_connections}, 总订阅: ${meter.total_subscriptions}`);
}

// 停止并释放
bridge.stop();
bridge.free();
```

## 注意事项

- OHOS 不允许在主线程进行长时间阻塞操作，建议使用 `taskpool` 将 NAPI 调用放在子线程执行
- Broker 端口号不要与设备上其他服务冲突（默认 1883）
- 生产环境建议启用 TLS 加密
- 确保 App 退出时正确调用 `stop()` 和 `free()` 释放资源
- 当前仅支持 `arm64-v8a` (aarch64) 架构，覆盖绝大多数 OHOS 真机设备
