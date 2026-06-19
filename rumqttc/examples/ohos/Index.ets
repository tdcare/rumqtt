/**
 * rumqttc OHOS 示例模块入口
 *
 * 导出 MQTT 客户端所需的全部公共 API：
 * - MQTT 客户端 (MqttClient)
 * - QoS 枚举 (MqttQos)
 * - 类型定义 (MqttEvent, MqttClientConfig)
 * - 版本查询 (getRumqttcClientVersion)
 */

// ===== MQTT 客户端 =====
export { MqttClient, MqttQos, getRumqttcClientVersion } from './ets/MqttClient';

export type { MqttEvent, MqttClientConfig } from './ets/MqttClient';
