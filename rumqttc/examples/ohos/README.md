# rumqttc OHOS NAPI 示例

将 [rumqttc](https://github.com/bytebeamio/rumqtt) 通过 [ohos-rs](https://ohos.rs) 封装为 OpenHarmony NAPI 原生模块，提供完整的 MQTT 3.1.1 协议支持，涵盖 TCP 和 WebSocket 传输。

## 环境要求

| 工具 | 版本要求 | 说明 |
|------|---------|------|
| Rust 工具链 | 1.88.0+ | MSRV，需与 ohos-rs 要求一致 |
| ohrs CLI | 最新版 | ohos-rs 构建工具，`cargo install ohrs` |
| OpenHarmony SDK | 5.0+ | 提供 OHOS 交叉编译 sysroot |
| DevEco Studio | 可选 | 用于 OHOS 应用开发和调试 |

## 编译步骤

### 方式一：使用构建脚本（推荐）

```powershell
cd rumqttc\src\ohos
.\build-rust.ps1
```

脚本会自动完成以下操作：

1. 调用 `ohrs build --release` 编译所有目标架构（arm64-v8a、armeabi-v7a、x86_64）
2. 将 `.so` 文件复制到 `libs/` 目录
3. 生成 TypeScript 类型定义文件到 `types/` 目录

### 方式二：手动构建

```powershell
cd rumqttc\src\ohos

# 编译 release 版本
ohrs build --release

# 产物位于 dist/ 目录
# dist/arm64-v8a/librumqttc_ohos_example.so
# dist/armeabi-v7a/librumqttc_ohos_example.so
# dist/x86_64/librumqttc_ohos_example.so
# dist/index.d.ts
```

构建产物说明：

```
src/ohos/
├── libs/
│   ├── arm64-v8a/librumqttc_ohos_example.so
│   ├── armeabi-v7a/librumqttc_ohos_example.so
│   └── x86_64/librumqttc_ohos_example.so
└── types/
    └── librumqttc_ohos_example.so.d.ts
```

## ArkTS 集成

### 接口定义

```typescript
/**
 * MQTT 客户端配置
 */
export interface MqttClientConfig {
  /** Broker 地址 (支持 tcp://, ws://, wss://, host:port, host) */
  brokerUrl: string;
  /** 客户端 ID */
  clientId: string;
  /** 用户名（可选） */
  username?: string;
  /** 密码（可选） */
  password?: string;
  /** 心跳间隔（秒），默认 60 */
  keepAliveSecs?: number;
  /** 是否清除会话，默认 true */
  cleanSession?: boolean;
}

/**
 * MQTT 事件类型
 */
export interface MqttEvent {
  type: 'connected' | 'disconnected' | 'message' | 'error';
  topic?: string;    // message 事件
  payload?: string;  // message 事件
  qos?: number;      // message 事件 (0, 1, 2)
  retain?: boolean;  // message 事件
  error?: string;    // error 事件
}

/**
 * QoS 级别
 */
export enum MqttQos {
  /** 最多一次 */
  AtMostOnce = 0,
  /** 至少一次 */
  AtLeastOnce = 1,
  /** 恰好一次 */
  ExactlyOnce = 2,
}
```

### MqttClient 包装类

```typescript
import { MqttClient as NativeMqttClient, getRumqttcVersion } from 'librumqttc_ohos_example.so';

export class MqttClient {
  private nativeClient: NativeMqttClient;
  private eventCallback?: (event: MqttEvent) => void;

  /**
   * 创建 MQTT 客户端（创建后自动连接并启动后台事件循环）
   */
  constructor(config: MqttClientConfig) {
    this.nativeClient = new NativeMqttClient(
      config.brokerUrl,
      config.clientId,
      config.username ?? null,
      config.password ?? null,
      config.keepAliveSecs ?? null,
      config.cleanSession ?? null
    );
  }

  /** 检查是否已连接 */
  isConnected(): boolean {
    return this.nativeClient.isConnected();
  }

  /** 订阅主题（支持通配符 + 和 #） */
  subscribe(topic: string, qos: MqttQos = MqttQos.AtMostOnce): boolean {
    return this.nativeClient.subscribe(topic, qos);
  }

  /** 取消订阅主题 */
  unsubscribe(topic: string): boolean {
    return this.nativeClient.unsubscribe(topic);
  }

  /** 发布消息 */
  publish(
    topic: string,
    payload: string,
    qos: MqttQos = MqttQos.AtMostOnce,
    retain: boolean = false
  ): boolean {
    return this.nativeClient.publish(topic, payload, qos, retain);
  }

  /** 断开连接 */
  disconnect(): boolean {
    return this.nativeClient.disconnect();
  }

  /** 轮询单个事件，无事件时返回 null */
  pollEvent(): MqttEvent | null {
    const json: string | null = this.nativeClient.pollEvent();
    if (json) {
      return JSON.parse(json) as MqttEvent;
    }
    return null;
  }

  /** 一次性取出所有待处理事件 */
  pollAllEvents(): MqttEvent[] {
    const jsons: string[] = this.nativeClient.pollAllEvents();
    return jsons.map((json: string) => JSON.parse(json) as MqttEvent);
  }

  /** 设置事件回调（需配合 processEvents() 使用） */
  onEvent(callback: (event: MqttEvent) => void): void {
    this.eventCallback = callback;
  }

  /** 处理待处理事件并触发回调，应在定时器中周期性调用 */
  processEvents(): void {
    if (this.eventCallback) {
      const events = this.pollAllEvents();
      for (const event of events) {
        this.eventCallback(event);
      }
    }
  }
}
```

### 使用示例

```typescript
import { MqttClient, MqttQos } from './MqttClient';

// 创建客户端（自动连接）
const client = new MqttClient({
  brokerUrl: 'ws://broker.emqx.io:8083/mqtt',
  clientId: 'ohos-client-001',
});

// 设置事件回调
client.onEvent((event) => {
  switch (event.type) {
    case 'connected':
      console.log('已连接到 MQTT Broker');
      client.subscribe('sensor/#', MqttQos.AtLeastOnce);
      break;
    case 'message':
      console.log(`收到消息: ${event.topic} = ${event.payload}`);
      break;
    case 'disconnected':
      console.log('已断开连接');
      break;
    case 'error':
      console.error(`错误: ${event.error}`);
      break;
  }
});

// 定时轮询事件（建议间隔 100ms）
setInterval(() => {
  client.processEvents();
}, 100);

// 发布消息
client.publish('sensor/temperature', '25.6', MqttQos.AtLeastOnce, false);

// 断开连接
// client.disconnect();
```

## 支持的 URL 格式

| 格式 | 示例 | 说明 |
|------|------|------|
| `tcp://host:port` | `tcp://192.168.1.100:1883` | TCP 连接，显式指定端口 |
| `ws://host:port/path` | `ws://broker.emqx.io:8083/mqtt` | WebSocket 连接 |
| `wss://host:port/path` | `wss://broker.emqx.io:8084/mqtt` | WebSocket over TLS |
| `host:port` | `192.168.1.100:1883` | TCP 连接（省略协议前缀） |
| `host` | `192.168.1.100` | TCP 连接，默认端口 1883 |

## API 说明

### MqttClient 类

#### `constructor(brokerUrl, clientId, username?, password?, keepAliveSecs?, cleanSession?)`

创建 MQTT 客户端并自动启动后台事件循环连接 Broker。

| 参数 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `brokerUrl` | `string` | 是 | — | Broker 地址 |
| `clientId` | `string` | 是 | — | 客户端标识 |
| `username` | `string \| null` | 否 | `null` | 认证用户名 |
| `password` | `string \| null` | 否 | `null` | 认证密码 |
| `keepAliveSecs` | `number \| null` | 否 | `60` | 心跳间隔（秒） |
| `cleanSession` | `boolean \| null` | 否 | `true` | 是否清除会话 |

#### `isConnected(): boolean`

返回客户端是否已连接到 Broker。

#### `subscribe(topic: string, qos: number): boolean`

订阅指定主题。`topic` 支持 MQTT 通配符（`+` 和 `#`），`qos` 取值 0/1/2。返回是否成功提交订阅请求。

#### `unsubscribe(topic: string): boolean`

取消订阅指定主题。返回是否成功提交取消订阅请求。

#### `publish(topic: string, payload: string, qos: number, retain: boolean): boolean`

发布消息到指定主题。返回是否成功提交发布请求。

| 参数 | 类型 | 说明 |
|------|------|------|
| `topic` | `string` | MQTT 主题 |
| `payload` | `string` | 消息内容 |
| `qos` | `number` | QoS 级别 (0, 1, 2) |
| `retain` | `boolean` | 是否保留消息 |

#### `disconnect(): boolean`

主动断开与 Broker 的连接。返回是否成功。

#### `pollEvent(): string | null`

从事件队列取出一条事件（JSON 字符串）。队列为空时返回 `null`。

#### `pollAllEvents(): string[]`

一次性取出所有待处理事件（JSON 字符串数组），减少跨语言调用次数。

### 工具函数

#### `getRumqttcVersion(): string`

返回 rumqttc 库版本信息，如 `"0.25.1-ohos"`。

## 注意事项

### INTERNET 权限

OHOS 应用需要网络权限才能连接 MQTT Broker。在 `module.json5` 的 `module` 节点中添加：

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

### 事件轮询建议

- 原生模块使用后台线程运行 rumqttc 事件循环，ArkTS 侧通过定时器轮询获取事件
- 推荐轮询间隔为 **100ms**（`setInterval(..., 100)`），兼顾实时性与性能
- 事件队列上限为 **50 条**，超出时最旧的事件会被丢弃
- 建议使用 `pollAllEvents()` 代替多次调用 `pollEvent()`，减少跨语言调用开销

### 自动重连

- 客户端内置自动重连机制，连接失败后会以指数退避策略重试（1s → 2s → ... → 5s）
- 连接错误事件不会洪泛推送，仅第 1 次和每第 10 次错误会产生事件通知

## License

Apache-2.0
