/**
 * MQTT 客户端封装
 *
 * 基于 Rust rumqttc 库的 OHOS NAPI 绑定，提供 MQTT 3.1.1 协议支持
 * 支持 TCP 和 WebSocket 连接
 */

import {
  MqttClient as NativeMqttClient,
  getRumqttcVersion,
} from 'librumqttc_ohos_example.so';

/**
 * MQTT 事件类型
 */
export interface MqttEvent {
  type: 'connected' | 'disconnected' | 'message' | 'error';
  topic?: string;
  payload?: string;
  qos?: number;
  retain?: boolean;
  error?: string;
}

/**
 * MQTT 客户端配置
 */
export interface MqttClientConfig {
  /** Broker 地址 (支持 tcp://, ws://, wss://) */
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

/**
 * MQTT 客户端
 *
 * 封装 Rust rumqttc 原生客户端，提供类型安全的 MQTT 操作
 *
 * @example
 * ```typescript
 * import { MqttClient, MqttQos } from './MqttClient';
 *
 * // 创建客户端
 * const client = new MqttClient({
 *   brokerUrl: 'ws://broker.emqx.io:8083/mqtt',
 *   clientId: 'ohos-client-001',
 * });
 *
 * // 注册事件回调
 * client.onEvent((event) => {
 *   if (event.type === 'message') {
 *     console.log(`收到消息: ${event.topic} = ${event.payload}`);
 *   }
 * });
 *
 * // 启动事件轮询
 * client.startEventLoop();
 *
 * // 订阅主题
 * client.subscribe('sensor/temperature', MqttQos.AtLeastOnce);
 *
 * // 发布消息
 * client.publish('sensor/temperature', '25.6', MqttQos.AtLeastOnce, false);
 * ```
 */
export class MqttClient {
  private nativeClient: NativeMqttClient;
  private eventCallback?: (event: MqttEvent) => void;
  private eventTimerId?: number;

  /**
   * 创建 MQTT 客户端
   *
   * 创建后自动连接并启动后台事件循环（Rust 侧）
   * ArkTS 侧需调用 startEventLoop() 开始定时轮询事件
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

  /**
   * 检查是否已连接
   */
  isConnected(): boolean {
    return this.nativeClient.isConnected();
  }

  /**
   * 订阅主题
   *
   * @param topic 主题名称（支持通配符 + 和 #）
   * @param qos QoS 级别，默认 AtMostOnce
   * @returns 是否成功发送订阅请求
   */
  subscribe(topic: string, qos: MqttQos = MqttQos.AtMostOnce): boolean {
    return this.nativeClient.subscribe(topic, qos);
  }

  /**
   * 取消订阅主题
   *
   * @param topic 主题名称
   * @returns 是否成功发送取消订阅请求
   */
  unsubscribe(topic: string): boolean {
    return this.nativeClient.unsubscribe(topic);
  }

  /**
   * 发布消息
   *
   * @param topic 主题名称
   * @param payload 消息内容
   * @param qos QoS 级别，默认 AtMostOnce
   * @param retain 是否保留消息，默认 false
   * @returns 是否成功发送发布请求
   */
  publish(
    topic: string,
    payload: string,
    qos: MqttQos = MqttQos.AtMostOnce,
    retain: boolean = false
  ): boolean {
    return this.nativeClient.publish(topic, payload, qos, retain);
  }

  /**
   * 断开连接
   *
   * 同时停止事件轮询定时器
   *
   * @returns 是否成功发送断开请求
   */
  disconnect(): boolean {
    this.stopEventLoop();
    return this.nativeClient.disconnect();
  }

  /**
   * 轮询单个事件
   *
   * @returns 事件对象，无事件时返回 null
   */
  pollEvent(): MqttEvent | null {
    const json: string | null = this.nativeClient.pollEvent();
    if (json) {
      return JSON.parse(json) as MqttEvent;
    }
    return null;
  }

  /**
   * 一次性取出所有待处理事件
   *
   * @returns 事件对象数组
   */
  pollAllEvents(): MqttEvent[] {
    const jsons: string[] = this.nativeClient.pollAllEvents();
    return jsons.map((json: string) => JSON.parse(json) as MqttEvent);
  }

  /**
   * 设置事件回调
   *
   * 需要配合 startEventLoop() 或手动调用 processEvents() 触发回调
   *
   * @example
   * ```typescript
   * client.onEvent((event) => {
   *   switch (event.type) {
   *     case 'connected':
   *       console.log('已连接到 broker');
   *       break;
   *     case 'message':
   *       console.log(`收到消息: ${event.topic} = ${event.payload}`);
   *       break;
   *     case 'disconnected':
   *       console.log('连接已断开');
   *       break;
   *     case 'error':
   *       console.error(`错误: ${event.error}`);
   *       break;
   *   }
   * });
   * ```
   */
  onEvent(callback: (event: MqttEvent) => void): void {
    this.eventCallback = callback;
  }

  /**
   * 处理待处理事件（触发回调）
   *
   * 可在定时器中周期性调用，或由 startEventLoop() 自动调用
   */
  processEvents(): void {
    if (this.eventCallback) {
      const events = this.pollAllEvents();
      for (const event of events) {
        this.eventCallback(event);
      }
    }
  }

  /**
   * 启动事件轮询定时器
   *
   * 自动定时调用 processEvents()，触发 onEvent 注册的回调
   *
   * @param intervalMs 轮询间隔（毫秒），默认 100ms
   */
  startEventLoop(intervalMs: number = 100): void {
    if (this.eventTimerId !== undefined) {
      return; // 已在运行，避免重复启动
    }
    this.eventTimerId = setInterval(() => {
      this.processEvents();
    }, intervalMs);
  }

  /**
   * 停止事件轮询定时器
   */
  stopEventLoop(): void {
    if (this.eventTimerId !== undefined) {
      clearInterval(this.eventTimerId);
      this.eventTimerId = undefined;
    }
  }
}

/**
 * 获取 rumqttc 库版本信息
 */
export function getRumqttcClientVersion(): string {
  return getRumqttcVersion();
}
