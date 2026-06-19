package com.example.rumqttc

import com.google.gson.annotations.SerializedName

/**
 * rumqttc FFI 返回的 JSON 数据模型
 *
 * 这些数据类用于解析 RumqttcBridge.pollEvent() / pollAllEvents()
 * 返回的 JSON 事件。字段名与 Rust 侧 serde 序列化的 JSON key 对应。
 */

// ========== MQTT 事件 ==========

/**
 * MQTT 事件
 *
 * 对应 rumqttc_poll_event() / rumqttc_poll_all_events() 返回的 JSON 对象。
 * type 字段决定事件类型：
 * - "connected"    — 已连接到 Broker
 * - "disconnected" — 与 Broker 断开连接
 * - "message"      — 收到订阅消息（此时 topic, payload, qos, retain 有效）
 * - "error"        — 发生错误（此时 error 有效）
 */
data class MqttEvent(
    /** 事件类型: "connected" | "disconnected" | "message" | "error" */
    val type: String,

    /** 消息主题（仅 type="message" 时有效） */
    val topic: String? = null,

    /** 消息内容（仅 type="message" 时有效） */
    val payload: String? = null,

    /** QoS 等级（仅 type="message" 时有效） */
    val qos: Int? = null,

    /** 是否为保留消息（仅 type="message" 时有效） */
    val retain: Boolean? = null,

    /** 错误信息（仅 type="error" 时有效） */
    val error: String? = null
)
