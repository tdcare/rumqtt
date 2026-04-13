package com.example.rumqttd

import android.app.Activity
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.*
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import java.util.concurrent.Executors

/**
 * MQTT Broker 控制界面
 *
 * 提供 Broker 的启动/停止控制，实时显示运行指标和客户端连接列表。
 * 所有 FFI 调用在子线程执行，UI 更新回主线程。
 */
class BrokerActivity : Activity() {

    companion object {
        private const val TAG = "BrokerActivity"

        /** 轮询间隔（毫秒） */
        private const val POLL_INTERVAL_MS = 2000L

        /**
         * 默认 TOML 配置
         * 仅开启 MQTT v4，监听 0.0.0.0:1883，不启用 TLS 和 WebSocket
         */
        private val DEFAULT_CONFIG = """
            id = 0

            [router]
            id = 0
            max_connections = 1010
            max_outgoing_packet_count = 200
            max_segment_size = 104857600
            max_segment_count = 10

            [v4.1]
            name = "v4-1"
            listen = "0.0.0.0:1883"
            next_connection_delay_ms = 1
                [v4.1.connections]
                connection_timeout_ms = 60000
                max_payload_size = 20480
                max_inflight_count = 100
                dynamic_filters = true

            [console]
            listen = "0.0.0.0:3030"
        """.trimIndent()
    }

    // ========== UI 控件 ==========
    private lateinit var btnToggle: Button
    private lateinit var tvStatus: TextView
    private lateinit var viewStatusDot: View
    private lateinit var tvConnections: TextView
    private lateinit var tvSubscriptions: TextView
    private lateinit var tvPublishes: TextView
    private lateinit var tvFailed: TextView
    private lateinit var lvConnections: ListView
    private lateinit var tvEmptyHint: TextView

    // ========== 状态 ==========
    private var isRunning = false
    private val handler = Handler(Looper.getMainLooper())
    private val executor = Executors.newSingleThreadExecutor()
    private val gson = Gson()
    private var connectionAdapter: ConnectionAdapter? = null

    // 最新的路由器指标（持续累积更新）
    private var latestMeter = RouterMeter()

    // ========== 轮询任务 ==========
    private val pollRunnable = object : Runnable {
        override fun run() {
            if (!isRunning) return
            pollBrokerStatus()
            handler.postDelayed(this, POLL_INTERVAL_MS)
        }
    }

    // ========== 生命周期 ==========

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_broker)

        initViews()
        setupListeners()
        updateUI(BrokerStatus())
    }

    override fun onDestroy() {
        super.onDestroy()
        handler.removeCallbacks(pollRunnable)
        if (isRunning) {
            executor.execute {
                RumqttdBridge.stop()
                RumqttdBridge.free()
            }
        }
        executor.shutdown()
    }

    // ========== 初始化 ==========

    private fun initViews() {
        btnToggle = findViewById(R.id.btn_toggle)
        tvStatus = findViewById(R.id.tv_status)
        viewStatusDot = findViewById(R.id.view_status_dot)
        tvConnections = findViewById(R.id.tv_connections)
        tvSubscriptions = findViewById(R.id.tv_subscriptions)
        tvPublishes = findViewById(R.id.tv_publishes)
        tvFailed = findViewById(R.id.tv_failed)
        lvConnections = findViewById(R.id.lv_connections)
        tvEmptyHint = findViewById(R.id.tv_empty_hint)

        connectionAdapter = ConnectionAdapter()
        lvConnections.adapter = connectionAdapter
    }

    private fun setupListeners() {
        btnToggle.setOnClickListener {
            if (isRunning) {
                stopBroker()
            } else {
                startBroker()
            }
        }
    }

    // ========== Broker 控制 ==========

    /**
     * 在子线程创建并启动 Broker
     */
    private fun startBroker() {
        btnToggle.isEnabled = false
        btnToggle.text = "启动中..."

        executor.execute {
            val created = RumqttdBridge.create(DEFAULT_CONFIG)
            if (!created) {
                val error = RumqttdBridge.getLastError() ?: "未知错误"
                Log.e(TAG, "创建 Broker 失败: $error")
                handler.post {
                    Toast.makeText(this, "创建失败: $error", Toast.LENGTH_LONG).show()
                    btnToggle.isEnabled = true
                    btnToggle.text = "开启"
                }
                return@execute
            }

            val started = RumqttdBridge.start()
            if (!started) {
                val error = RumqttdBridge.getLastError() ?: "未知错误"
                Log.e(TAG, "启动 Broker 失败: $error")
                RumqttdBridge.free()
                handler.post {
                    Toast.makeText(this, "启动失败: $error", Toast.LENGTH_LONG).show()
                    btnToggle.isEnabled = true
                    btnToggle.text = "开启"
                }
                return@execute
            }

            Log.i(TAG, "Broker 启动成功，监听 0.0.0.0:1883")
            handler.post {
                isRunning = true
                latestMeter = RouterMeter()
                btnToggle.isEnabled = true
                btnToggle.text = "关闭"
                updateStatusIndicator(true)
                // 开始轮询
                handler.postDelayed(pollRunnable, POLL_INTERVAL_MS)
            }
        }
    }

    /**
     * 停止 Broker 并释放资源
     */
    private fun stopBroker() {
        btnToggle.isEnabled = false
        btnToggle.text = "停止中..."
        handler.removeCallbacks(pollRunnable)

        executor.execute {
            RumqttdBridge.stop()
            RumqttdBridge.free()
            Log.i(TAG, "Broker 已停止")

            handler.post {
                isRunning = false
                btnToggle.isEnabled = true
                btnToggle.text = "开启"
                updateStatusIndicator(false)
                updateUI(BrokerStatus())
            }
        }
    }

    // ========== 数据轮询 ==========

    /**
     * 在子线程轮询 Broker 状态，解析后回主线程更新 UI
     */
    private fun pollBrokerStatus() {
        executor.execute {
            try {
                // 获取连接信息
                val connectionsJson = RumqttdBridge.getConnections()
                val connections: List<ConnectionInfo> = if (connectionsJson != null) {
                    val type = object : TypeToken<List<ConnectionInfo>>() {}.type
                    gson.fromJson(connectionsJson, type) ?: emptyList()
                } else {
                    emptyList()
                }

                // 获取路由器指标
                val metersJson = RumqttdBridge.getMeters()
                if (metersJson != null && metersJson != "[]") {
                    // 尝试解析最新的路由器指标
                    try {
                        val type = object : TypeToken<List<Map<String, Any>>>() {}.type
                        val meters: List<Map<String, Any>> = gson.fromJson(metersJson, type) ?: emptyList()
                        // 从指标数组中查找最新的路由器指标
                        for (meter in meters) {
                            val routerData = meter["Router"] as? Map<*, *>
                            if (routerData != null) {
                                latestMeter = RouterMeter(
                                    totalConnections = (routerData["total_connections"] as? Double)?.toInt() ?: latestMeter.totalConnections,
                                    totalSubscriptions = (routerData["total_subscriptions"] as? Double)?.toInt() ?: latestMeter.totalSubscriptions,
                                    totalPublishes = (routerData["total_publishes"] as? Double)?.toInt() ?: latestMeter.totalPublishes,
                                    failedPublishes = (routerData["failed_publishes"] as? Double)?.toInt() ?: latestMeter.failedPublishes
                                )
                            }
                        }
                    } catch (e: Exception) {
                        Log.w(TAG, "解析指标 JSON 异常: ${e.message}")
                    }
                }

                val status = BrokerStatus(
                    isRunning = true,
                    connectionCount = latestMeter.totalConnections,
                    subscriptionCount = latestMeter.totalSubscriptions,
                    totalPublishes = latestMeter.totalPublishes,
                    failedPublishes = latestMeter.failedPublishes,
                    connections = connections
                )

                handler.post { updateUI(status) }

            } catch (e: Exception) {
                Log.e(TAG, "轮询异常: ${e.message}", e)
            }
        }
    }

    // ========== UI 更新 ==========

    private fun updateStatusIndicator(running: Boolean) {
        if (running) {
            tvStatus.text = "运行中"
            tvStatus.setTextColor(Color.parseColor("#4CAF50"))
            viewStatusDot.setBackgroundColor(Color.parseColor("#4CAF50"))
        } else {
            tvStatus.text = "已停止"
            tvStatus.setTextColor(Color.parseColor("#F44336"))
            viewStatusDot.setBackgroundColor(Color.parseColor("#F44336"))
        }
    }

    private fun updateUI(status: BrokerStatus) {
        tvConnections.text = status.connectionCount.toString()
        tvSubscriptions.text = status.subscriptionCount.toString()
        tvPublishes.text = status.totalPublishes.toString()
        tvFailed.text = status.failedPublishes.toString()

        connectionAdapter?.updateData(status.connections)

        if (status.connections.isEmpty()) {
            tvEmptyHint.visibility = View.VISIBLE
            lvConnections.visibility = View.GONE
        } else {
            tvEmptyHint.visibility = View.GONE
            lvConnections.visibility = View.VISIBLE
        }
    }

    // ========== 连接列表适配器 ==========

    /**
     * ListView 适配器，显示客户端连接列表
     */
    private inner class ConnectionAdapter : BaseAdapter() {

        private var data: List<ConnectionInfo> = emptyList()

        fun updateData(newData: List<ConnectionInfo>) {
            data = newData
            notifyDataSetChanged()
        }

        override fun getCount(): Int = data.size
        override fun getItem(position: Int): ConnectionInfo = data[position]
        override fun getItemId(position: Int): Long = position.toLong()

        override fun getView(position: Int, convertView: View?, parent: ViewGroup?): View {
            val view = convertView ?: LayoutInflater.from(this@BrokerActivity)
                .inflate(R.layout.item_connection, parent, false)

            val item = data[position]

            val tvClientId = view.findViewById<TextView>(R.id.tv_client_id)
            val tvSubCount = view.findViewById<TextView>(R.id.tv_sub_count)
            val tvIncoming = view.findViewById<TextView>(R.id.tv_incoming)
            val tvOutgoing = view.findViewById<TextView>(R.id.tv_outgoing)
            val tvConnStatus = view.findViewById<TextView>(R.id.tv_conn_status)

            tvClientId.text = item.clientId
            tvSubCount.text = "订阅: ${item.subscriptions.size}"
            tvIncoming.text = "入站: ${item.incomingPublishCount}"
            tvOutgoing.text = "出站: ${item.outgoingPublishCount}"
            tvConnStatus.text = item.status

            return view
        }
    }
}
