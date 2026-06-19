package com.example.rumqttc

import android.app.Activity
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.View
import android.widget.*
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import java.text.SimpleDateFormat
import java.util.*
import java.util.concurrent.Executors

/**
 * MQTT 客户端控制界面
 *
 * 提供 MQTT 客户端的连接/断开、订阅/发布控制，实时显示收到的消息和状态日志。
 * 所有 FFI 调用在子线程执行，UI 更新回主线程。
 */
class MqttClientActivity : Activity() {

    companion object {
        private const val TAG = "MqttClientActivity"

        /** 事件轮询间隔（毫秒） */
        private const val POLL_INTERVAL_MS = 200L

        /** 日志最大行数，超出后清除旧日志 */
        private const val MAX_LOG_LINES = 200

        /** 消息列表最大条数 */
        private const val MAX_MESSAGES = 100
    }

    // ========== UI 控件 ==========
    private lateinit var etBrokerUrl: EditText
    private lateinit var etClientId: EditText
    private lateinit var etUsername: EditText
    private lateinit var etPassword: EditText
    private lateinit var btnConnect: Button
    private lateinit var viewStatusDot: View
    private lateinit var tvStatus: TextView

    private lateinit var etSubTopic: EditText
    private lateinit var spinnerSubQos: Spinner
    private lateinit var btnSubscribe: Button
    private lateinit var btnUnsubscribe: Button

    private lateinit var etPubTopic: EditText
    private lateinit var etPubPayload: EditText
    private lateinit var spinnerPubQos: Spinner
    private lateinit var cbRetain: CheckBox
    private lateinit var btnPublish: Button

    private lateinit var llMessages: LinearLayout
    private lateinit var svMessages: ScrollView
    private lateinit var tvEmptyMessages: TextView

    private lateinit var tvLog: TextView
    private lateinit var svLog: ScrollView

    // ========== 状态 ==========
    private var isConnected = false
    private val mainHandler = Handler(Looper.getMainLooper())
    private val executor = Executors.newSingleThreadExecutor()
    private val gson = Gson()
    private val timeFormat = SimpleDateFormat("HH:mm:ss", Locale.getDefault())
    private var messageCount = 0
    private var logLineCount = 0

    // ========== 事件轮询任务 ==========
    private val pollRunnable = object : Runnable {
        override fun run() {
            if (!RumqttcBridge.isCreated()) return
            pollEvents()
            mainHandler.postDelayed(this, POLL_INTERVAL_MS)
        }
    }

    // ========== 生命周期 ==========

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_mqtt_client)

        initViews()
        setupListeners()
        updateConnectionUI(false)
    }

    override fun onDestroy() {
        super.onDestroy()
        mainHandler.removeCallbacks(pollRunnable)
        if (RumqttcBridge.isCreated()) {
            executor.execute {
                RumqttcBridge.disconnect()
                RumqttcBridge.free()
            }
        }
        executor.shutdown()
    }

    // ========== 初始化 ==========

    private fun initViews() {
        // 连接配置
        etBrokerUrl = findViewById(R.id.et_broker_url)
        etClientId = findViewById(R.id.et_client_id)
        etUsername = findViewById(R.id.et_username)
        etPassword = findViewById(R.id.et_password)
        btnConnect = findViewById(R.id.btn_connect)
        viewStatusDot = findViewById(R.id.view_status_dot)
        tvStatus = findViewById(R.id.tv_status)

        // 订阅区
        etSubTopic = findViewById(R.id.et_sub_topic)
        spinnerSubQos = findViewById(R.id.spinner_sub_qos)
        btnSubscribe = findViewById(R.id.btn_subscribe)
        btnUnsubscribe = findViewById(R.id.btn_unsubscribe)

        // 发布区
        etPubTopic = findViewById(R.id.et_pub_topic)
        etPubPayload = findViewById(R.id.et_pub_payload)
        spinnerPubQos = findViewById(R.id.spinner_pub_qos)
        cbRetain = findViewById(R.id.cb_retain)
        btnPublish = findViewById(R.id.btn_publish)

        // 消息列表
        llMessages = findViewById(R.id.ll_messages)
        svMessages = findViewById(R.id.sv_messages)
        tvEmptyMessages = findViewById(R.id.tv_empty_messages)

        // 日志区
        tvLog = findViewById(R.id.tv_log)
        svLog = findViewById(R.id.sv_log)

        // QoS Spinner 适配器
        val qosAdapter = ArrayAdapter(
            this,
            android.R.layout.simple_spinner_item,
            arrayOf("QoS 0", "QoS 1", "QoS 2")
        )
        qosAdapter.setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item)
        spinnerSubQos.adapter = qosAdapter
        spinnerPubQos.adapter = qosAdapter
    }

    private fun setupListeners() {
        btnConnect.setOnClickListener {
            if (isConnected || RumqttcBridge.isCreated()) {
                disconnectClient()
            } else {
                connectClient()
            }
        }

        btnSubscribe.setOnClickListener {
            val topic = etSubTopic.text.toString().trim()
            if (topic.isEmpty()) {
                Toast.makeText(this, "请输入订阅主题", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val qos = spinnerSubQos.selectedItemPosition
            executor.execute {
                val ok = RumqttcBridge.subscribe(topic, qos)
                mainHandler.post {
                    if (ok) {
                        appendLog("订阅成功: $topic (QoS $qos)")
                    } else {
                        val err = RumqttcBridge.getLastError() ?: "未知错误"
                        appendLog("订阅失败: $topic - $err")
                    }
                }
            }
        }

        btnUnsubscribe.setOnClickListener {
            val topic = etSubTopic.text.toString().trim()
            if (topic.isEmpty()) {
                Toast.makeText(this, "请输入取消订阅主题", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            executor.execute {
                val ok = RumqttcBridge.unsubscribe(topic)
                mainHandler.post {
                    if (ok) {
                        appendLog("取消订阅: $topic")
                    } else {
                        val err = RumqttcBridge.getLastError() ?: "未知错误"
                        appendLog("取消订阅失败: $topic - $err")
                    }
                }
            }
        }

        btnPublish.setOnClickListener {
            val topic = etPubTopic.text.toString().trim()
            val payload = etPubPayload.text.toString()
            if (topic.isEmpty()) {
                Toast.makeText(this, "请输入发布主题", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val qos = spinnerPubQos.selectedItemPosition
            val retain = cbRetain.isChecked
            executor.execute {
                val ok = RumqttcBridge.publish(topic, payload, qos, retain)
                mainHandler.post {
                    if (ok) {
                        appendLog("发布: $topic (QoS $qos, retain=$retain)")
                    } else {
                        val err = RumqttcBridge.getLastError() ?: "未知错误"
                        appendLog("发布失败: $topic - $err")
                    }
                }
            }
        }
    }

    // ========== 连接控制 ==========

    /**
     * 在子线程创建客户端并连接
     */
    private fun connectClient() {
        val brokerUrl = etBrokerUrl.text.toString().trim()
        val clientId = etClientId.text.toString().trim()
        val username = etUsername.text.toString().trim().ifEmpty { null }
        val password = etPassword.text.toString().trim().ifEmpty { null }

        if (brokerUrl.isEmpty() || clientId.isEmpty()) {
            Toast.makeText(this, "请输入 Broker 地址和客户端 ID", Toast.LENGTH_SHORT).show()
            return
        }

        btnConnect.isEnabled = false
        btnConnect.text = "连接中..."
        appendLog("正在连接 $brokerUrl ...")

        executor.execute {
            val created = RumqttcBridge.create(
                brokerUrl = brokerUrl,
                clientId = clientId,
                username = username,
                password = password
            )

            if (!created) {
                val error = RumqttcBridge.getLastError() ?: "未知错误"
                Log.e(TAG, "创建客户端失败: $error")
                mainHandler.post {
                    appendLog("连接失败: $error")
                    Toast.makeText(this, "连接失败: $error", Toast.LENGTH_LONG).show()
                    btnConnect.isEnabled = true
                    btnConnect.text = "连接"
                }
                return@execute
            }

            Log.i(TAG, "客户端已创建，连接 $brokerUrl")
            mainHandler.post {
                btnConnect.isEnabled = true
                btnConnect.text = "断开"
                updateConnectionUI(true)
                appendLog("客户端已创建，等待连接...")
                // 开始轮询事件
                mainHandler.postDelayed(pollRunnable, POLL_INTERVAL_MS)
            }
        }
    }

    /**
     * 断开连接并释放资源
     */
    private fun disconnectClient() {
        btnConnect.isEnabled = false
        btnConnect.text = "断开中..."
        mainHandler.removeCallbacks(pollRunnable)

        executor.execute {
            RumqttcBridge.disconnect()
            RumqttcBridge.free()
            Log.i(TAG, "客户端已断开")

            mainHandler.post {
                isConnected = false
                btnConnect.isEnabled = true
                btnConnect.text = "连接"
                updateConnectionUI(false)
                appendLog("已断开连接")
            }
        }
    }

    // ========== 事件轮询 ==========

    /**
     * 在子线程轮询所有待处理事件，解析后回主线程更新 UI
     */
    private fun pollEvents() {
        executor.execute {
            try {
                val eventsJson = RumqttcBridge.pollAllEvents() ?: return@execute
                if (eventsJson == "[]") return@execute

                val type = object : TypeToken<List<MqttEvent>>() {}.type
                val events: List<MqttEvent> = gson.fromJson(eventsJson, type) ?: return@execute

                for (event in events) {
                    mainHandler.post { handleEvent(event) }
                }
            } catch (e: Exception) {
                Log.e(TAG, "轮询事件异常: ${e.message}", e)
            }
        }
    }

    /**
     * 处理单个 MQTT 事件（主线程）
     */
    private fun handleEvent(event: MqttEvent) {
        when (event.type) {
            "connected" -> {
                isConnected = true
                updateStatusIndicator(true)
                appendLog("已连接到 Broker")
            }
            "disconnected" -> {
                isConnected = false
                updateStatusIndicator(false)
                appendLog("与 Broker 断开连接")
            }
            "message" -> {
                val topic = event.topic ?: ""
                val payload = event.payload ?: ""
                val qos = event.qos ?: 0
                val retain = event.retain ?: false
                addMessage(topic, payload, qos, retain)
            }
            "error" -> {
                val error = event.error ?: "未知错误"
                appendLog("错误: $error")
            }
            else -> {
                appendLog("未知事件: ${event.type}")
            }
        }
    }

    // ========== UI 更新 ==========

    private fun updateConnectionUI(connected: Boolean) {
        val enabled = connected || RumqttcBridge.isCreated()
        btnSubscribe.isEnabled = enabled
        btnUnsubscribe.isEnabled = enabled
        btnPublish.isEnabled = enabled
        updateStatusIndicator(connected)
    }

    private fun updateStatusIndicator(connected: Boolean) {
        if (connected) {
            tvStatus.text = "已连接"
            tvStatus.setTextColor(Color.parseColor("#4CAF50"))
            viewStatusDot.setBackgroundColor(Color.parseColor("#4CAF50"))
        } else {
            tvStatus.text = if (RumqttcBridge.isCreated()) "连接中" else "未连接"
            val color = if (RumqttcBridge.isCreated()) "#FF9800" else "#F44336"
            tvStatus.setTextColor(Color.parseColor(color))
            viewStatusDot.setBackgroundColor(Color.parseColor(color))
        }
    }

    /**
     * 添加收到的消息到消息列表
     */
    private fun addMessage(topic: String, payload: String, qos: Int, retain: Boolean) {
        tvEmptyMessages.visibility = View.GONE
        svMessages.visibility = View.VISIBLE

        // 限制最大消息数
        if (messageCount >= MAX_MESSAGES) {
            if (llMessages.childCount > 0) {
                llMessages.removeViewAt(0)
            }
        } else {
            messageCount++
        }

        val time = timeFormat.format(Date())
        val tv = TextView(this).apply {
            val retainTag = if (retain) " [R]" else ""
            text = "[$time] [$topic] (QoS $qos$retainTag)\n$payload"
            textSize = 13f
            setTextColor(Color.parseColor("#333333"))
            setPadding(12, 8, 12, 8)
            setBackgroundColor(Color.parseColor(if (messageCount % 2 == 0) "#FFFFFF" else "#F8F8F8"))
        }
        llMessages.addView(tv)

        // 自动滚动到底部
        svMessages.post { svMessages.fullScroll(View.FOCUS_DOWN) }

        appendLog("收到消息: $topic (QoS $qos)")
    }

    /**
     * 追加日志
     */
    private fun appendLog(msg: String) {
        val time = timeFormat.format(Date())
        val line = "[$time] $msg\n"

        logLineCount++
        if (logLineCount > MAX_LOG_LINES) {
            // 清除前半部分日志
            val current = tvLog.text.toString()
            val halfIdx = current.length / 2
            val newStart = current.indexOf('\n', halfIdx)
            if (newStart > 0) {
                tvLog.text = current.substring(newStart + 1)
                logLineCount = tvLog.text.count { it == '\n' }
            }
        }

        tvLog.append(line)
        svLog.post { svLog.fullScroll(View.FOCUS_DOWN) }
    }
}
