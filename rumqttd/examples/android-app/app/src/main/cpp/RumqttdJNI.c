/**
 * rumqttd JNI 胶水层
 *
 * 将 Kotlin/Java 的 native 方法桥接到 rumqttd C FFI 函数。
 * JNI 方法名格式：Java_<包名>_<类名>_<方法名>（点号替换为下划线）
 *
 * 编译方式：
 *   使用 Android NDK 交叉编译，链接 librumqttd.so
 *   或通过 CMakeLists.txt 集成到 Android 构建系统
 */

#include <jni.h>
#include <stdint.h>
#include <string.h>
#include "rumqttd.h"

/* ========== 辅助函数 ========== */

/**
 * 将 C 字符串转换为 Java String
 * 如果 c_str 为 NULL，返回 NULL
 */
static jstring c_str_to_jstring(JNIEnv *env, const char *c_str) {
    if (c_str == NULL) {
        return NULL;
    }
    return (*env)->NewStringUTF(env, c_str);
}

/* ========== JNI 方法实现 ========== */

/**
 * 创建 broker 实例
 *
 * 将 Java String 配置转为 C 字符串，调用 rumqttd_create()，
 * 返回 broker 指针作为 jlong 句柄。
 *
 * @return broker 指针（jlong），失败返回 0
 */
JNIEXPORT jlong JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeCreate(
        JNIEnv *env, jobject thiz, jstring config_toml) {
    const char *c_config = (*env)->GetStringUTFChars(env, config_toml, NULL);
    if (c_config == NULL) {
        return 0; // OutOfMemoryError 已由 JVM 抛出
    }

    RumqttdBroker *broker = rumqttd_create(c_config);
    (*env)->ReleaseStringUTFChars(env, config_toml, c_config);

    return (jlong)(intptr_t)broker;
}

/**
 * 启动 broker（非阻塞）
 *
 * @return 0 = 成功，-1 = 失败
 */
JNIEXPORT jint JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeStart(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttdBroker *broker = (RumqttdBroker *)(intptr_t)handle;
    return (jint)rumqttd_start(broker);
}

/**
 * 停止 broker
 *
 * @return 0 = 成功，-1 = 失败
 */
JNIEXPORT jint JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeStop(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttdBroker *broker = (RumqttdBroker *)(intptr_t)handle;
    return (jint)rumqttd_stop(broker);
}

/**
 * 释放 broker 实例
 *
 * 调用后 handle 不可再使用。传入 0（NULL）安全忽略。
 */
JNIEXPORT void JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeFree(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttdBroker *broker = (RumqttdBroker *)(intptr_t)handle;
    rumqttd_free(broker);
}

/**
 * 获取最近一次错误信息
 *
 * 返回的字符串指针由 thread_local 管理，无需调用 rumqttd_free_string。
 * 指针在下一次 FFI 调用前有效。
 *
 * @return 错误信息字符串，无错误时返回 NULL
 */
JNIEXPORT jstring JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeLastError(
        JNIEnv *env, jobject thiz) {
    const char *error = rumqttd_last_error();
    return c_str_to_jstring(env, error);
    // 注意：不调用 rumqttd_free_string，因为 last_error 是 thread_local 管理的
}

/**
 * 获取活跃连接信息（JSON 数组）
 *
 * 调用 rumqttd_get_connections() 获取 JSON 字符串，
 * 转为 Java String 后立即调用 rumqttd_free_string() 释放 C 侧内存。
 *
 * @return JSON 字符串，失败返回 NULL
 */
JNIEXPORT jstring JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeGetConnections(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttdBroker *broker = (RumqttdBroker *)(intptr_t)handle;
    char *json = rumqttd_get_connections(broker);
    if (json == NULL) {
        return NULL;
    }

    jstring result = (*env)->NewStringUTF(env, json);
    rumqttd_free_string(json); // 释放 Rust 分配的字符串
    return result;
}

/**
 * 获取路由器指标（JSON 数组，非阻塞）
 *
 * @return JSON 字符串，无新数据时返回 "[]"，失败返回 NULL
 */
JNIEXPORT jstring JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeGetMeters(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttdBroker *broker = (RumqttdBroker *)(intptr_t)handle;
    char *json = rumqttd_get_meters(broker);
    if (json == NULL) {
        return NULL;
    }

    jstring result = (*env)->NewStringUTF(env, json);
    rumqttd_free_string(json); // 释放 Rust 分配的字符串
    return result;
}

/**
 * 获取告警信息（JSON 数组，非阻塞）
 *
 * @return JSON 字符串，无新告警时返回 "[]"，失败返回 NULL
 */
JNIEXPORT jstring JNICALL
Java_com_example_rumqttd_RumqttdBridge_nativeGetAlerts(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttdBroker *broker = (RumqttdBroker *)(intptr_t)handle;
    char *json = rumqttd_get_alerts(broker);
    if (json == NULL) {
        return NULL;
    }

    jstring result = (*env)->NewStringUTF(env, json);
    rumqttd_free_string(json); // 释放 Rust 分配的字符串
    return result;
}
