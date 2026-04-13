package com.example.rumqttd

/**
 * rumqttd MQTT Broker 的 JNI 封装
 *
 * 单例类，封装 librumqttd.so 提供的所有 native 方法。
 * 内部维护一个 broker 句柄（C 指针），通过 jlong 传递。
 *
 * 使用流程：
 * ```
 * val bridge = RumqttdBridge
 * bridge.create(configToml)   // 创建 broker 实例
 * bridge.start()              // 在后台线程启动 broker
 * val connections = bridge.getConnections()  // 获取连接信息 JSON
 * val meters = bridge.getMeters()            // 获取路由器指标 JSON
 * bridge.stop()               // 停止 broker
 * bridge.free()               // 释放资源
 * ```
 */
object RumqttdBridge {

    init {
        System.loadLibrary("rumqttd_jni")  // 唯一需要加载的库，已静态包含 rumqttd
    }

    // ========== Native 方法声明（对应 rumqttd.h 中的 C FFI 函数）==========

    /** 创建 broker 实例，返回不透明指针（作为 Long 句柄） */
    private external fun nativeCreate(configToml: String): Long

    /** 在后台线程启动 broker，0 = 成功，-1 = 失败 */
    private external fun nativeStart(handle: Long): Int

    /** 停止 broker，0 = 成功，-1 = 失败 */
    private external fun nativeStop(handle: Long): Int

    /** 释放 broker 实例，之后句柄不可再使用 */
    private external fun nativeFree(handle: Long)

    /** 获取最近一次错误信息（线程局部，无需释放） */
    private external fun nativeLastError(): String?

    /** 获取活跃连接信息（JSON 数组），JNI 层自动调用 rumqttd_free_string */
    private external fun nativeGetConnections(handle: Long): String?

    /** 获取路由器指标（JSON 数组），JNI 层自动调用 rumqttd_free_string */
    private external fun nativeGetMeters(handle: Long): String?

    /** 获取告警信息（JSON 数组），JNI 层自动调用 rumqttd_free_string */
    private external fun nativeGetAlerts(handle: Long): String?

    // 注意：rumqttd_free_string 在 JNI C 层自动处理，不需要暴露给 Kotlin

    // ========== Broker 句柄 ==========

    @Volatile
    private var brokerHandle: Long = 0L

    // ========== 公开 API ==========

    /**
     * 创建 broker 实例
     *
     * @param configToml TOML 格式的配置字符串
     * @return true 表示创建成功，false 表示失败（可通过 getLastError() 获取原因）
     */
    fun create(configToml: String): Boolean {
        brokerHandle = nativeCreate(configToml)
        return brokerHandle != 0L
    }

    /**
     * 启动 broker（非阻塞，broker 在后台线程运行）
     *
     * @return true 表示启动成功
     */
    fun start(): Boolean = nativeStart(brokerHandle) == 0

    /**
     * 停止 broker
     *
     * @return true 表示停止成功
     */
    fun stop(): Boolean = nativeStop(brokerHandle) == 0

    /**
     * 释放 broker 实例，释放后不可再调用其他方法
     */
    fun free() {
        if (brokerHandle != 0L) {
            nativeFree(brokerHandle)
            brokerHandle = 0L
        }
    }

    /**
     * 获取活跃连接信息
     *
     * @return JSON 数组字符串，每个元素包含 connection_id, client_id, subscriptions 等字段；
     *         失败返回 null
     */
    fun getConnections(): String? = nativeGetConnections(brokerHandle)

    /**
     * 获取路由器指标（非阻塞）
     *
     * @return JSON 数组字符串，无新数据时返回 "[]"；失败返回 null
     */
    fun getMeters(): String? = nativeGetMeters(brokerHandle)

    /**
     * 获取告警信息（非阻塞）
     *
     * @return JSON 数组字符串，无新告警时返回 "[]"；失败返回 null
     */
    fun getAlerts(): String? = nativeGetAlerts(brokerHandle)

    /**
     * 获取最近一次错误信息
     *
     * @return 错误描述字符串，无错误时返回 null
     */
    fun getLastError(): String? = nativeLastError()

    /**
     * 检查 broker 实例是否已创建
     */
    fun isCreated(): Boolean = brokerHandle != 0L
}
