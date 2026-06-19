/**
 * rumqttc - MQTT Client Library for Android
 *
 * C FFI bindings for the rumqttc MQTT client.
 * This header provides functions to create, manage, and communicate with
 * an MQTT broker from C/C++ or Android JNI code.
 *
 * Thread Safety: All functions are safe to call from multiple threads
 * concurrently on the same client instance. Internal locking is used
 * to prevent data races.
 */

#ifndef RUMQTTC_H
#define RUMQTTC_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque handle to a rumqttc client instance.
 * Created by rumqttc_create(), freed by rumqttc_free().
 */
typedef struct RumqttcClient RumqttcClient;

/**
 * Create a new MQTT client and start the background event loop.
 *
 * The client will automatically attempt to connect to the broker and
 * maintain the connection (with automatic reconnection on failure).
 *
 * @param broker_url       Broker URL: "tcp://host:port", "ws://host:port/path", or "wss://..."
 * @param client_id        MQTT client identifier (must not be NULL).
 * @param username         Optional username for authentication (NULL if not needed).
 * @param password         Optional password for authentication (NULL if not needed).
 * @param keep_alive_secs  Keep-alive interval in seconds (0 = default 60s).
 * @param clean_session    1 = clean session, 0 = persistent session.
 * @return Pointer to client instance on success, NULL on failure.
 *         Call rumqttc_last_error() to get the error message on failure.
 */
RumqttcClient* rumqttc_create(const char* broker_url,
                               const char* client_id,
                               const char* username,
                               const char* password,
                               uint32_t keep_alive_secs,
                               int clean_session);

/**
 * Free a client instance and release all associated resources.
 *
 * After this call the pointer is invalid and must not be used.
 * Passing NULL is safely ignored.
 *
 * @param client  Pointer to client instance, or NULL.
 */
void rumqttc_free(RumqttcClient* client);

/**
 * Subscribe to a topic.
 *
 * @param client  Pointer to client instance (must not be NULL).
 * @param topic   MQTT topic string (supports wildcards + and #).
 * @param qos     Quality of Service level (0, 1, or 2).
 * @return 0 on success, -1 on failure.
 */
int rumqttc_subscribe(RumqttcClient* client, const char* topic, int qos);

/**
 * Unsubscribe from a topic.
 *
 * @param client  Pointer to client instance (must not be NULL).
 * @param topic   MQTT topic string to unsubscribe from.
 * @return 0 on success, -1 on failure.
 */
int rumqttc_unsubscribe(RumqttcClient* client, const char* topic);

/**
 * Publish a message to a topic.
 *
 * @param client       Pointer to client instance (must not be NULL).
 * @param topic        MQTT topic string.
 * @param payload      Message payload bytes (NULL allowed if payload_len is 0).
 * @param payload_len  Length of payload in bytes.
 * @param qos          Quality of Service level (0, 1, or 2).
 * @param retain       1 = retain message, 0 = don't retain.
 * @return 0 on success, -1 on failure.
 */
int rumqttc_publish(RumqttcClient* client,
                    const char* topic,
                    const uint8_t* payload,
                    uint32_t payload_len,
                    int qos,
                    int retain);

/**
 * Disconnect from the broker.
 *
 * @param client  Pointer to client instance (must not be NULL).
 * @return 0 on success, -1 on failure.
 */
int rumqttc_disconnect(RumqttcClient* client);

/**
 * Check if the client is currently connected.
 *
 * @param client  Pointer to client instance (must not be NULL).
 * @return 1 if connected, 0 if disconnected, -1 on error.
 */
int rumqttc_is_connected(RumqttcClient* client);

/**
 * Poll a single event from the event queue.
 *
 * Events are JSON objects with a "type" field:
 * - {"type":"connected"}
 * - {"type":"disconnected"}
 * - {"type":"message","topic":"...","payload":"...","qos":0,"retain":false}
 * - {"type":"error","error":"..."}
 *
 * @param client  Pointer to client instance (must not be NULL).
 * @return JSON string on success (must be freed with rumqttc_free_string),
 *         NULL if no event is available.
 */
char* rumqttc_poll_event(RumqttcClient* client);

/**
 * Poll all pending events from the event queue at once.
 *
 * Returns a JSON array of event objects. Returns "[]" if no events.
 *
 * @param client  Pointer to client instance (must not be NULL).
 * @return JSON array string (must be freed with rumqttc_free_string),
 *         NULL on error.
 */
char* rumqttc_poll_all_events(RumqttcClient* client);

/**
 * Free a string returned by rumqttc_poll_event or rumqttc_poll_all_events.
 *
 * @param s  String pointer to free. NULL is safely ignored.
 */
void rumqttc_free_string(char* s);

/**
 * Get the last error message.
 *
 * Returns a pointer to a null-terminated UTF-8 string describing the last error.
 * The returned pointer is valid until the next FFI call on the same thread.
 * The caller must NOT free this pointer.
 *
 * @return Error message string, or NULL if no error has occurred.
 */
const char* rumqttc_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* RUMQTTC_H */
