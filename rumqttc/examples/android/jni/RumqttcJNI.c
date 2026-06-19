/**
 * rumqttc JNI 胶水层
 *
 * 将 Kotlin/Java 的 native 方法桥接到 rumqttc C FFI 函数。
 * JNI 方法名格式：Java_<包名>_<类名>_<方法名>（点号替换为下划线）
 *
 * 编译方式：
 *   使用 Android NDK + CMake，链接 librumqttc_android.a 静态库
 *   参见 CMakeLists.txt
 */

#include <jni.h>
#include <stdint.h>
#include <string.h>
#include "rumqttc.h"

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

/**
 * 将 Rust 返回的 char* 转为 Java String，然后立即释放 C 侧内存
 * 用于 rumqttc_poll_event / rumqttc_poll_all_events 返回的字符串
 */
static jstring rust_str_to_jstring_and_free(JNIEnv *env, char *rust_str) {
    if (rust_str == NULL) {
        return NULL;
    }
    jstring result = (*env)->NewStringUTF(env, rust_str);
    rumqttc_free_string(rust_str);
    return result;
}

/* ========== JNI 方法实现 ========== */

/**
 * 创建 MQTT 客户端实例
 *
 * 将 Java String 参数转为 C 字符串，调用 rumqttc_create()，
 * 返回客户端指针作为 jlong 句柄。
 *
 * @return 客户端指针（jlong），失败返回 0
 */
JNIEXPORT jlong JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativeCreate(
        JNIEnv *env, jobject thiz,
        jstring broker_url, jstring client_id,
        jstring username, jstring password,
        jint keep_alive_secs, jboolean clean_session) {

    const char *c_url = (*env)->GetStringUTFChars(env, broker_url, NULL);
    if (c_url == NULL) return 0;

    const char *c_client_id = (*env)->GetStringUTFChars(env, client_id, NULL);
    if (c_client_id == NULL) {
        (*env)->ReleaseStringUTFChars(env, broker_url, c_url);
        return 0;
    }

    /* username 和 password 可选，Java 侧可能传 null */
    const char *c_username = NULL;
    if (username != NULL) {
        c_username = (*env)->GetStringUTFChars(env, username, NULL);
    }

    const char *c_password = NULL;
    if (password != NULL) {
        c_password = (*env)->GetStringUTFChars(env, password, NULL);
    }

    RumqttcClient *client = rumqttc_create(
        c_url, c_client_id, c_username, c_password,
        (uint32_t)keep_alive_secs,
        clean_session ? 1 : 0
    );

    /* 释放所有 JNI 字符串 */
    (*env)->ReleaseStringUTFChars(env, broker_url, c_url);
    (*env)->ReleaseStringUTFChars(env, client_id, c_client_id);
    if (c_username != NULL) {
        (*env)->ReleaseStringUTFChars(env, username, c_username);
    }
    if (c_password != NULL) {
        (*env)->ReleaseStringUTFChars(env, password, c_password);
    }

    return (jlong)(intptr_t)client;
}

/**
 * 释放客户端实例
 *
 * 调用后 handle 不可再使用。传入 0（NULL）安全忽略。
 */
JNIEXPORT void JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativeFree(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;
    rumqttc_free(client);
}

/**
 * 订阅主题
 *
 * @return 0 = 成功，-1 = 失败
 */
JNIEXPORT jint JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativeSubscribe(
        JNIEnv *env, jobject thiz, jlong handle,
        jstring topic, jint qos) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;

    const char *c_topic = (*env)->GetStringUTFChars(env, topic, NULL);
    if (c_topic == NULL) return -1;

    int result = rumqttc_subscribe(client, c_topic, (int)qos);
    (*env)->ReleaseStringUTFChars(env, topic, c_topic);
    return (jint)result;
}

/**
 * 取消订阅主题
 *
 * @return 0 = 成功，-1 = 失败
 */
JNIEXPORT jint JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativeUnsubscribe(
        JNIEnv *env, jobject thiz, jlong handle, jstring topic) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;

    const char *c_topic = (*env)->GetStringUTFChars(env, topic, NULL);
    if (c_topic == NULL) return -1;

    int result = rumqttc_unsubscribe(client, c_topic);
    (*env)->ReleaseStringUTFChars(env, topic, c_topic);
    return (jint)result;
}

/**
 * 发布消息
 *
 * 将 Java String payload 转为 UTF-8 字节数组传递给 FFI
 *
 * @return 0 = 成功，-1 = 失败
 */
JNIEXPORT jint JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativePublish(
        JNIEnv *env, jobject thiz, jlong handle,
        jstring topic, jstring payload, jint qos, jboolean retain) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;

    const char *c_topic = (*env)->GetStringUTFChars(env, topic, NULL);
    if (c_topic == NULL) return -1;

    const char *c_payload = NULL;
    uint32_t payload_len = 0;
    if (payload != NULL) {
        c_payload = (*env)->GetStringUTFChars(env, payload, NULL);
        if (c_payload != NULL) {
            payload_len = (uint32_t)strlen(c_payload);
        }
    }

    int result = rumqttc_publish(
        client, c_topic,
        (const uint8_t *)c_payload, payload_len,
        (int)qos,
        retain ? 1 : 0
    );

    (*env)->ReleaseStringUTFChars(env, topic, c_topic);
    if (c_payload != NULL) {
        (*env)->ReleaseStringUTFChars(env, payload, c_payload);
    }
    return (jint)result;
}

/**
 * 断开连接
 *
 * @return 0 = 成功，-1 = 失败
 */
JNIEXPORT jint JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativeDisconnect(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;
    return (jint)rumqttc_disconnect(client);
}

/**
 * 检查是否已连接
 *
 * @return 1 = 已连接，0 = 未连接，-1 = 错误
 */
JNIEXPORT jint JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativeIsConnected(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;
    return (jint)rumqttc_is_connected(client);
}

/**
 * 轮询单个事件
 *
 * 从事件队列获取一个事件（JSON 对象），无事件时返回 NULL。
 * JNI 层自动调用 rumqttc_free_string 释放 Rust 侧内存。
 *
 * @return JSON 字符串，无事件返回 NULL
 */
JNIEXPORT jstring JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativePollEvent(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;
    char *json = rumqttc_poll_event(client);
    return rust_str_to_jstring_and_free(env, json);
}

/**
 * 轮询所有待处理事件
 *
 * 一次性获取事件队列中的所有事件（JSON 数组）。
 * 无事件时返回 "[]"。
 * JNI 层自动调用 rumqttc_free_string 释放 Rust 侧内存。
 *
 * @return JSON 数组字符串，错误返回 NULL
 */
JNIEXPORT jstring JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativePollAllEvents(
        JNIEnv *env, jobject thiz, jlong handle) {
    RumqttcClient *client = (RumqttcClient *)(intptr_t)handle;
    char *json = rumqttc_poll_all_events(client);
    return rust_str_to_jstring_and_free(env, json);
}

/**
 * 获取最近一次错误信息
 *
 * 返回的字符串指针由 thread_local 管理，无需调用 rumqttc_free_string。
 * 指针在下一次 FFI 调用前有效。
 *
 * @return 错误信息字符串，无错误时返回 NULL
 */
JNIEXPORT jstring JNICALL
Java_com_example_rumqttc_RumqttcBridge_nativeLastError(
        JNIEnv *env, jobject thiz) {
    const char *error = rumqttc_last_error();
    return c_str_to_jstring(env, error);
    /* 注意：不调用 rumqttc_free_string，因为 last_error 是 thread_local 管理的 */
}
