package com.example.rumqttd

import com.google.gson.annotations.SerializedName

/**
 * rumqttd FFI 返回的 JSON 数据模型
 *
 * 这些数据类用于解析 RumqttdBridge.getConnections()、getMeters()、getAlerts()
 * 返回的 JSON 字符串。字段名通过 @SerializedName 与 Rust 侧 serde 序列化的 JSON key 对应。
 */

// ========== 连接信息 ==========

/**
 * 单个客户端连接的信息
 *
 * 对应 rumqttd_get_connections() 返回的 JSON 数组中的每个元素。
 */
data class ConnectionInfo(
    /** 内部连接 ID */
    @SerializedName("connection_id")
    val connectionId: Int,

    /** MQTT 客户端标识符 */
    @SerializedName("client_id")
    val clientId: String,

    /** 该客户端订阅的 topic filter 列表 */
    val subscriptions: List<String>,

    /** 入站发布消息计数 */
    @SerializedName("incoming_publish_count")
    val incomingPublishCount: Int,

    /** 入站发布消息总字节数 */
    @SerializedName("incoming_publish_size")
    val incomingPublishSize: Int,

    /** 出站发布消息计数 */
    @SerializedName("outgoing_publish_count")
    val outgoingPublishCount: Int,

    /** 出站发布消息总字节数 */
    @SerializedName("outgoing_publish_size")
    val outgoingPublishSize: Int,

    /**
     * 连接状态的调试表示
     * 例如："Ready"、"Paused(Caughtup)"
     * 注意：格式在不同版本间不保证稳定
     */
    val status: String,

    /** 客户端 IP 地址 */
    @SerializedName("peer_addr")
    val peerAddr: String = ""
)

// ========== 路由器指标 ==========

/**
 * 路由器级别的运行指标
 *
 * 对应 rumqttd_get_meters() 返回的 JSON 中 Router 类型的指标数据。
 * Rust 侧 Meter 是枚举类型，JSON 序列化格式取决于 serde 配置。
 */
data class RouterMeter(
    /** 时间戳（Unix 毫秒） */
    val timestamp: Long = 0,

    /** 序列号 */
    val sequence: Int = 0,

    /** 路由器 ID */
    @SerializedName("router_id")
    val routerId: Int = 0,

    /** 当前总连接数 */
    @SerializedName("total_connections")
    val totalConnections: Int = 0,

    /** 当前总订阅数 */
    @SerializedName("total_subscriptions")
    val totalSubscriptions: Int = 0,

    /** 累计发布消息数 */
    @SerializedName("total_publishes")
    val totalPublishes: Int = 0,

    /** 累计失败发布数 */
    @SerializedName("failed_publishes")
    val failedPublishes: Int = 0
)

// ========== 订阅指标 ==========

/**
 * 订阅级别的指标
 *
 * 对应 Meter::Subscription 变体的数据。
 */
data class SubscriptionMeter(
    /** Topic filter */
    val filter: String = "",

    /** 匹配该 filter 的消息计数 */
    @SerializedName("publish_count")
    val publishCount: Int = 0,

    /** 匹配该 filter 的消息总字节数 */
    @SerializedName("publish_size")
    val publishSize: Int = 0
)

// ========== 告警信息 ==========

/**
 * 告警信息
 *
 * 对应 rumqttd_get_alerts() 返回的 JSON 数组中的元素。
 * 具体字段取决于 Rust 侧 Alert 的序列化格式。
 */
data class AlertInfo(
    /** 告警类型/标题 */
    val title: String = "",

    /** 告警详情 */
    val message: String = "",

    /** 时间戳 */
    val timestamp: Long = 0
)

// ========== 汇总数据（UI 层使用）==========

/**
 * Broker 运行状态汇总，供 UI 层使用
 */
data class BrokerStatus(
    /** 是否正在运行 */
    val isRunning: Boolean = false,

    /** 当前连接数 */
    val connectionCount: Int = 0,

    /** 当前订阅数 */
    val subscriptionCount: Int = 0,

    /** 累计发布消息数 */
    val totalPublishes: Int = 0,

    /** 累计失败消息数 */
    val failedPublishes: Int = 0,

    /** 连接列表 */
    val connections: List<ConnectionInfo> = emptyList()
)
