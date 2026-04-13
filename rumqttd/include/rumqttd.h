/**
 * rumqttd - MQTT Broker Library for Android
 * 
 * C FFI bindings for the rumqttd MQTT broker.
 * This header provides functions to create, start, stop, and manage
 * an MQTT broker instance from C/C++ or Android JNI code.
 *
 * Thread Safety: All functions are safe to call from multiple threads
 * concurrently on the same broker instance. Internal locking is used
 * to prevent data races.
 */

#ifndef RUMQTTD_H
#define RUMQTTD_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque handle to a rumqttd broker instance.
 * Created by rumqttd_create(), freed by rumqttd_free().
 */
typedef struct RumqttdBroker RumqttdBroker;

/**
 * Create a new broker instance from a TOML configuration string.
 *
 * @param config_toml  Null-terminated UTF-8 TOML configuration string.
 * @return Pointer to broker instance on success, NULL on failure.
 *         Call rumqttd_last_error() to get the error message on failure.
 */
RumqttdBroker* rumqttd_create(const char* config_toml);

/**
 * Start the broker in a background thread (non-blocking).
 *
 * The broker will listen for MQTT connections according to its configuration.
 * This function returns immediately; the broker runs in a separate thread.
 *
 * @param broker  Pointer to broker instance (must not be NULL).
 * @return 0 on success, -1 on failure.
 *         Call rumqttd_last_error() to get the error message on failure.
 */
int rumqttd_start(RumqttdBroker* broker);

/**
 * Stop a running broker.
 *
 * NOTE: Currently this marks the broker as stopped and detaches the background
 * thread, but does NOT perform a graceful shutdown of internal server threads.
 * The broker thread resources may not be fully released until process exit.
 *
 * @param broker  Pointer to broker instance (must not be NULL).
 * @return 0 on success, -1 on failure.
 *         Call rumqttd_last_error() to get the error message on failure.
 */
int rumqttd_stop(RumqttdBroker* broker);

/**
 * Free a broker instance and release associated Rust resources.
 *
 * NOTE: If the broker is still running, this will release the Rust-side handle
 * but internal server threads may continue until process exit.
 * Call rumqttd_stop() first if possible.
 *
 * After this call, the broker pointer is invalid and must not be used.
 *
 * @param broker  Pointer to broker instance. NULL is safely ignored.
 */
void rumqttd_free(RumqttdBroker* broker);

/**
 * Get the last error message.
 *
 * Returns a pointer to a null-terminated UTF-8 string describing the last error.
 * The returned pointer is valid until the next FFI call on the same thread.
 *
 * @return Error message string, or NULL if no error has occurred.
 */
const char* rumqttd_last_error(void);

/**
 * Get active connections as a JSON string.
 *
 * Returns a JSON array of connection objects, each containing:
 * - connection_id: number
 * - client_id: string
 * - subscriptions: string array
 * - incoming_publish_count: number
 * - incoming_publish_size: number
 * - outgoing_publish_count: number
 * - outgoing_publish_size: number
 * - status: string (debug representation of internal state, e.g. "Ready",
 *           "Paused(Caughtup)". Format is not guaranteed stable across versions)
 *
 * @param broker  Pointer to broker instance (must not be NULL).
 * @return JSON string on success (must be freed with rumqttd_free_string), NULL on failure.
 */
char* rumqttd_get_connections(RumqttdBroker* broker);

/**
 * Get router metrics as a JSON string (non-blocking).
 *
 * Returns a JSON array of metric objects. If no new data is available,
 * returns "[]".
 *
 * @param broker  Pointer to broker instance (must not be NULL).
 * @return JSON string on success (must be freed with rumqttd_free_string), NULL on failure.
 */
char* rumqttd_get_meters(RumqttdBroker* broker);

/**
 * Get alerts as a JSON string (non-blocking).
 *
 * Returns a JSON array of alert objects. If no new alerts are available,
 * returns "[]".
 *
 * @param broker  Pointer to broker instance (must not be NULL).
 * @return JSON string on success (must be freed with rumqttd_free_string), NULL on failure.
 */
char* rumqttd_get_alerts(RumqttdBroker* broker);

/**
 * Free a string returned by rumqttd_get_connections, rumqttd_get_meters,
 * or rumqttd_get_alerts.
 *
 * @param s  String pointer to free. NULL is safely ignored.
 */
void rumqttd_free_string(char* s);

#ifdef __cplusplus
}
#endif

#endif /* RUMQTTD_H */
