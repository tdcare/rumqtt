/**
 * rumqttd MQTT Broker 的 NAPI 封装
 *
 * 封装 librumqttd_napi.so 提供的所有原生方法。
 * 内部维护一个 RumqttdBroker 实例，通过 ohos-rs 生成的 NAPI 接口调用。
 *
 * 使用流程：
 * ```
 * const bridge = new RumqttdBridge();
 * bridge.create(configToml);           // 创建 broker 实例
 * bridge.start();                      // 启动 broker
 * const conns = bridge.getConnections(); // 获取连接信息
 * const meter = bridge.getMeters();      // 获取路由器指标
 * bridge.stop();                       // 停止 broker
 * bridge.free();                       // 释放资源
 * ```
 */
import { hilog } from '@kit.PerformanceAnalysisKit';
import { RumqttdBroker } from 'librumqttd_napi.so';
import { ConnectionInfo, RouterMeter, AlertInfo, MeterEntry } from './BrokerModels';

const TAG = 'RumqttdBridge';
const DOMAIN = 0x0000;

export class RumqttdBridge {
  // ========== Broker 实例 ==========

  private broker: RumqttdBroker | null = null;
  private running: boolean = false;

  // ========== Getter ==========

  /**
   * 检查 broker 实例是否已创建
   */
  get isCreated(): boolean {
    return this.broker !== null;
  }

  /**
   * 检查 broker 是否正在运行
   */
  get isRunning(): boolean {
    return this.running;
  }

  // ========== 公开 API ==========

  /**
   * 创建 broker 实例
   *
   * @param configToml TOML 格式的配置字符串
   * @returns true 表示创建成功，false 表示失败
   */
  create(configToml: string): boolean {
    try {
      this.broker = new RumqttdBroker(configToml);
      hilog.info(DOMAIN, TAG, 'Broker 实例创建成功');
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `创建 Broker 失败: ${(error as Error).message}`);
      this.broker = null;
      return false;
    }
  }

  /**
   * 启动 broker（非阻塞，broker 在后台线程运行）
   *
   * @returns true 表示启动成功
   */
  start(): boolean {
    if (this.broker === null) {
      hilog.error(DOMAIN, TAG, '启动失败: Broker 未创建');
      return false;
    }
    try {
      this.broker.start();
      this.running = true;
      hilog.info(DOMAIN, TAG, 'Broker 启动成功');
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `启动 Broker 失败: ${(error as Error).message}`);
      return false;
    }
  }

  /**
   * 停止 broker
   *
   * @returns true 表示停止成功
   */
  stop(): boolean {
    if (this.broker === null) {
      hilog.warn(DOMAIN, TAG, '停止失败: Broker 未创建');
      return false;
    }
    try {
      this.broker.stop();
      this.running = false;
      hilog.info(DOMAIN, TAG, 'Broker 已停止');
      return true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `停止 Broker 失败: ${(error as Error).message}`);
      return false;
    }
  }

  /**
   * 释放 broker 实例，释放后不可再调用其他方法
   */
  free(): void {
    if (this.broker !== null) {
      try {
        this.broker.free();
        hilog.info(DOMAIN, TAG, 'Broker 资源已释放');
      } catch (error) {
        hilog.error(DOMAIN, TAG, `释放 Broker 失败: ${(error as Error).message}`);
      } finally {
        this.broker = null;
        this.running = false;
      }
    }
  }

  /**
   * 获取活跃连接信息
   *
   * @returns 连接信息数组；失败返回空数组
   */
  getConnections(): ConnectionInfo[] {
    if (this.broker === null) {
      return [];
    }
    try {
      const json = this.broker.getConnections();
      if (json === null || json === undefined || json === '') {
        return [];
      }
      const connections: ConnectionInfo[] = JSON.parse(json) as ConnectionInfo[];
      return connections;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `获取连接信息失败: ${(error as Error).message}`);
      return [];
    }
  }

  /**
   * 获取路由器指标
   *
   * 从 meters JSON 数组中查找最新的 Router 类型指标。
   *
   * @returns RouterMeter 或 null（无数据时）
   */
  getMeters(): RouterMeter | null {
    if (this.broker === null) {
      return null;
    }
    try {
      const json = this.broker.getMeters();
      if (json === null || json === undefined || json === '' || json === '[]') {
        return null;
      }
      const meters: MeterEntry[] = JSON.parse(json) as MeterEntry[];
      // 从指标数组中查找最新的路由器指标
      for (let i = meters.length - 1; i >= 0; i--) {
        const entry = meters[i];
        if (entry.Router !== undefined && entry.Router !== null) {
          return entry.Router;
        }
      }
      return null;
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `解析指标 JSON 异常: ${(error as Error).message}`);
      return null;
    }
  }

  /**
   * 获取告警信息
   *
   * @returns 告警信息数组；失败返回空数组
   */
  getAlerts(): AlertInfo[] {
    if (this.broker === null) {
      return [];
    }
    try {
      const json = this.broker.getAlerts();
      if (json === null || json === undefined || json === '') {
        return [];
      }
      const alerts: AlertInfo[] = JSON.parse(json) as AlertInfo[];
      return alerts;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `获取告警信息失败: ${(error as Error).message}`);
      return [];
    }
  }
}
