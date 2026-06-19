package com.example.rumqttc

/**
 * rumqttc MQTT Client 的 JNI 封装
 *
 * 单例类，封装 librumqttc_jni.so 提供的所有 native 方法。
 * 内部维护一个 client 句柄（C 指针），通过 jlong 传递。
 *
 * 使用流程：
 * ```
 * val bridge = RumqttcBridge
 * bridge.create("tcp://192.168.1.48:1883", "my-client")  // 创建并连接
 * bridge.subscribe("test/topic", 1)                       // 订阅主题
 * bridge.publish("test/topic", "hello", 1, false)         // 发布消息
 * val events = bridge.pollAllEvents()                     // 轮询事件
 * bridge.disconnect()                                     // 断开连接
 * bridge.free()                                           // 释放资源
 * ```
 */
object RumqttcBridge {

    init {
        System.loadLibrary("rumqttc_jni")  // 唯一需要加载的库，已静态包含 rumqttc
    }

    // ========== Native 方法声明（对应 rumqttc.h 中的 C FFI 函数）==========

    /** 创建客户端实例，返回不透明指针（作为 Long 句柄） */
    private external fun nativeCreate(
        brokerUrl: String, clientId: String,
        username: String?, password: String?,
        keepAliveSecs: Int, cleanSession: Boolean
    ): Long

    /** 释放客户端实例，之后句柄不可再使用 */
    private external fun nativeFree(handle: Long)

    /** 订阅主题，0 = 成功，-1 = 失败 */
    private external fun nativeSubscribe(handle: Long, topic: String, qos: Int): Int

    /** 取消订阅主题，0 = 成功，-1 = 失败 */
    private external fun nativeUnsubscribe(handle: Long, topic: String): Int

    /** 发布消息，0 = 成功，-1 = 失败 */
    private external fun nativePublish(
        handle: Long, topic: String, payload: String,
        qos: Int, retain: Boolean
    ): Int

    /** 断开连接，0 = 成功，-1 = 失败 */
    private external fun nativeDisconnect(handle: Long): Int

    /** 检查是否已连接，1 = 已连接，0 = 未连接，-1 = 错误 */
    private external fun nativeIsConnected(handle: Long): Int

    /** 轮询单个事件（JSON 对象），无事件返回 null */
    private external fun nativePollEvent(handle: Long): String?

    /** 轮询所有待处理事件（JSON 数组），无事件返回 "[]" */
    private external fun nativePollAllEvents(handle: Long): String?

    /** 获取最近一次错误信息（线程局部，无需释放） */
    private external fun nativeLastError(): String?

    // 注意：rumqttc_free_string 在 JNI C 层自动处理，不需要暴露给 Kotlin

    // ========== Client 句柄 ==========

    @Volatile
    private var clientHandle: Long = 0L

    // ========== 公开 API ==========

    /**
     * 创建 MQTT 客户端并启动后台事件循环
     *
     * 客户端会自动尝试连接 Broker，断线后自动重连。
     *
     * @param brokerUrl      Broker 地址，如 "tcp://host:port"
     * @param clientId       MQTT 客户端标识符
     * @param username       可选的用户名（null 表示不使用）
     * @param password       可选的密码（null 表示不使用）
     * @param keepAliveSecs  心跳间隔秒数（0 = 默认 60s）
     * @param cleanSession   是否使用 clean session
     * @return true 表示创建成功，false 表示失败（可通过 getLastError() 获取原因）
     */
    fun create(
        brokerUrl: String,
        clientId: String,
        username: String? = null,
        password: String? = null,
        keepAliveSecs: Int = 60,
        cleanSession: Boolean = true
    ): Boolean {
        clientHandle = nativeCreate(brokerUrl, clientId, username, password, keepAliveSecs, cleanSession)
        return clientHandle != 0L
    }

    /**
     * 释放客户端实例，释放后不可再调用其他方法
     */
    fun free() {
        if (clientHandle != 0L) {
            nativeFree(clientHandle)
            clientHandle = 0L
        }
    }

    /**
     * 订阅主题
     *
     * @param topic  MQTT topic（支持通配符 + 和 #）
     * @param qos    QoS 等级（0, 1, 2）
     * @return true 表示订阅请求已发送
     */
    fun subscribe(topic: String, qos: Int = 0): Boolean =
        nativeSubscribe(clientHandle, topic, qos) == 0

    /**
     * 取消订阅主题
     *
     * @param topic  MQTT topic
     * @return true 表示取消订阅请求已发送
     */
    fun unsubscribe(topic: String): Boolean =
        nativeUnsubscribe(clientHandle, topic) == 0

    /**
     * 发布消息
     *
     * @param topic    MQTT topic
     * @param payload  消息内容
     * @param qos      QoS 等级（0, 1, 2）
     * @param retain   是否保留消息
     * @return true 表示发布请求已发送
     */
    fun publish(topic: String, payload: String, qos: Int = 0, retain: Boolean = false): Boolean =
        nativePublish(clientHandle, topic, payload, qos, retain) == 0

    /**
     * 断开与 Broker 的连接
     *
     * @return true 表示断开成功
     */
    fun disconnect(): Boolean = nativeDisconnect(clientHandle) == 0

    /**
     * 检查客户端是否已连接
     *
     * @return true 表示已连接
     */
    fun isConnected(): Boolean = nativeIsConnected(clientHandle) == 1

    /**
     * 轮询单个事件
     *
     * @return JSON 字符串（事件对象），无事件返回 null
     */
    fun pollEvent(): String? = nativePollEvent(clientHandle)

    /**
     * 轮询所有待处理事件
     *
     * @return JSON 数组字符串，无事件返回 "[]"；错误返回 null
     */
    fun pollAllEvents(): String? = nativePollAllEvents(clientHandle)

    /**
     * 获取最近一次错误信息
     *
     * @return 错误描述字符串，无错误时返回 null
     */
    fun getLastError(): String? = nativeLastError()

    /**
     * 检查客户端实例是否已创建
     */
    fun isCreated(): Boolean = clientHandle != 0L
}
