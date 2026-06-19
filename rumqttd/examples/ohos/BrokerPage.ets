/**
 * MQTT Broker 控制界面
 *
 * 提供 Broker 的启动/停止控制，实时显示运行指标和客户端连接列表。
 * 所有 NAPI 调用通过 taskpool 在子线程执行，UI 更新回主线程。
 *
 * 对应 Android 端的 BrokerActivity.kt + activity_broker.xml + item_connection.xml
 */
import { hilog } from '@kit.PerformanceAnalysisKit';
import { ConnectionInfo, BrokerStatus } from './BrokerModels';
import { RumqttdBridge } from './RumqttdBridge';

const TAG = 'BrokerPage';
const DOMAIN = 0x0000;

/** 轮询间隔（毫秒） */
const POLL_INTERVAL_MS = 2000;

/**
 * 默认 TOML 配置
 * 仅开启 MQTT v4，监听 0.0.0.0:1883，不启用 TLS 和 WebSocket
 */
const DEFAULT_CONFIG = `id = 0

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
listen = "0.0.0.0:3030"`;

@Entry
@Component
struct BrokerPage {
  // ========== 状态变量 ==========

  /** Broker 是否正在运行 */
  @State isRunning: boolean = false;
  /** 按钮是否可用 */
  @State btnEnabled: boolean = true;
  /** 按钮文本 */
  @State btnText: string = '开启';
  /** 状态文本 */
  @State statusText: string = '已停止';
  /** 状态颜色 */
  @State statusColor: string = '#F44336';
  /** 总连接数 */
  @State connectionCount: number = 0;
  /** 总订阅数 */
  @State subscriptionCount: number = 0;
  /** 发布消息数 */
  @State totalPublishes: number = 0;
  /** 失败消息数 */
  @State failedPublishes: number = 0;
  /** 客户端连接列表 */
  @State connections: ConnectionInfo[] = [];

  // ========== 私有属性 ==========

  /** Bridge 实例 */
  private bridge: RumqttdBridge = new RumqttdBridge();
  /** 轮询定时器 ID */
  private pollTimer: number = -1;

  // ========== 生命周期 ==========

  aboutToDisappear(): void {
    this.stopPolling();
    if (this.isRunning) {
      this.bridge.stop();
      this.bridge.free();
    }
  }

  // ========== Broker 控制 ==========

  /**
   * 启动 Broker
   */
  private startBroker(): void {
    this.btnEnabled = false;
    this.btnText = '启动中...';

    // 创建 broker 实例
    const created = this.bridge.create(DEFAULT_CONFIG);
    if (!created) {
      hilog.error(DOMAIN, TAG, '创建 Broker 失败');
      this.btnEnabled = true;
      this.btnText = '开启';
      return;
    }

    // 启动 broker
    const started = this.bridge.start();
    if (!started) {
      hilog.error(DOMAIN, TAG, '启动 Broker 失败');
      this.bridge.free();
      this.btnEnabled = true;
      this.btnText = '开启';
      return;
    }

    hilog.info(DOMAIN, TAG, 'Broker 启动成功，监听 0.0.0.0:1883');
    this.isRunning = true;
    this.btnEnabled = true;
    this.btnText = '关闭';
    this.updateStatusIndicator(true);

    // 开始轮询
    this.startPolling();
  }

  /**
   * 停止 Broker 并释放资源
   */
  private stopBroker(): void {
    this.btnEnabled = false;
    this.btnText = '停止中...';
    this.stopPolling();

    this.bridge.stop();
    this.bridge.free();
    hilog.info(DOMAIN, TAG, 'Broker 已停止');

    this.isRunning = false;
    this.btnEnabled = true;
    this.btnText = '开启';
    this.updateStatusIndicator(false);
    this.resetMetrics();
  }

  // ========== 数据轮询 ==========

  /**
   * 开始定时轮询
   */
  private startPolling(): void {
    this.pollTimer = setInterval(() => {
      this.pollBrokerStatus();
    }, POLL_INTERVAL_MS);
  }

  /**
   * 停止定时轮询
   */
  private stopPolling(): void {
    if (this.pollTimer !== -1) {
      clearInterval(this.pollTimer);
      this.pollTimer = -1;
    }
  }

  /**
   * 轮询 Broker 状态，更新 UI
   */
  private pollBrokerStatus(): void {
    if (!this.isRunning) {
      return;
    }

    try {
      // 获取连接信息
      const conns = this.bridge.getConnections();
      this.connections = conns;

      // 获取路由器指标
      const meter = this.bridge.getMeters();
      if (meter !== null) {
        this.connectionCount = meter.total_connections;
        this.subscriptionCount = meter.total_subscriptions;
        this.totalPublishes = meter.total_publishes;
        this.failedPublishes = meter.failed_publishes;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `轮询异常: ${(error as Error).message}`);
    }
  }

  // ========== UI 辅助 ==========

  /**
   * 更新状态指示器
   */
  private updateStatusIndicator(running: boolean): void {
    if (running) {
      this.statusText = '运行中';
      this.statusColor = '#4CAF50';
    } else {
      this.statusText = '已停止';
      this.statusColor = '#F44336';
    }
  }

  /**
   * 重置指标到初始值
   */
  private resetMetrics(): void {
    this.connectionCount = 0;
    this.subscriptionCount = 0;
    this.totalPublishes = 0;
    this.failedPublishes = 0;
    this.connections = [];
  }

  // ========== UI 构建 ==========

  build() {
    Column() {
      // ==================== 顶部标题栏 ====================
      Row() {
        Text('RUMQTTD Broker')
          .fontSize(20)
          .fontColor(Color.White)
          .fontWeight(FontWeight.Bold)
      }
      .width('100%')
      .height(56)
      .backgroundColor('#1976D2')
      .justifyContent(FlexAlign.Center)
      .shadow({
        radius: 4,
        color: 'rgba(0, 0, 0, 0.2)',
        offsetX: 0,
        offsetY: 2
      })

      // ==================== 内容区域 ====================
      Scroll() {
        Column() {

          // ========== 控制区域 ==========
          Row() {
            // 开启/关闭按钮
            Button(this.btnText)
              .fontSize(16)
              .fontColor(Color.White)
              .backgroundColor('#1976D2')
              .height(44)
              .padding({ left: 32, right: 32 })
              .borderRadius(4)
              .enabled(this.btnEnabled)
              .opacity(this.btnEnabled ? 1.0 : 0.6)
              .onClick(() => {
                if (this.isRunning) {
                  this.stopBroker();
                } else {
                  this.startBroker();
                }
              })

            Blank()

            // 状态指示圆点
            Circle()
              .width(12)
              .height(12)
              .fill(this.statusColor)
              .margin({ right: 8 })

            // 状态文本
            Text(this.statusText)
              .fontSize(16)
              .fontColor(this.statusColor)
          }
          .width('100%')
          .alignItems(VerticalAlign.Center)
          .margin({ bottom: 16 })

          // ========== 运行指标卡片 ==========
          Column() {
            // 卡片标题
            Text('运行指标')
              .fontSize(16)
              .fontWeight(FontWeight.Bold)
              .fontColor('#333333')
              .margin({ bottom: 12 })
              .alignSelf(ItemAlign.Start)

            // 第一行：连接数 + 订阅数
            Row() {
              // 总连接数
              Column() {
                Text(this.connectionCount.toString())
                  .fontSize(28)
                  .fontWeight(FontWeight.Bold)
                  .fontColor('#1976D2')
                Text('总连接数')
                  .fontSize(12)
                  .fontColor('#999999')
                  .margin({ top: 4 })
              }
              .layoutWeight(1)
              .alignItems(HorizontalAlign.Center)

              // 总订阅数
              Column() {
                Text(this.subscriptionCount.toString())
                  .fontSize(28)
                  .fontWeight(FontWeight.Bold)
                  .fontColor('#1976D2')
                Text('总订阅数')
                  .fontSize(12)
                  .fontColor('#999999')
                  .margin({ top: 4 })
              }
              .layoutWeight(1)
              .alignItems(HorizontalAlign.Center)
            }
            .width('100%')
            .margin({ bottom: 8 })

            // 第二行：发布数 + 失败数
            Row() {
              // 发布消息
              Column() {
                Text(this.totalPublishes.toString())
                  .fontSize(28)
                  .fontWeight(FontWeight.Bold)
                  .fontColor('#4CAF50')
                Text('发布消息')
                  .fontSize(12)
                  .fontColor('#999999')
                  .margin({ top: 4 })
              }
              .layoutWeight(1)
              .alignItems(HorizontalAlign.Center)

              // 失败消息
              Column() {
                Text(this.failedPublishes.toString())
                  .fontSize(28)
                  .fontWeight(FontWeight.Bold)
                  .fontColor('#F44336')
                Text('失败消息')
                  .fontSize(12)
                  .fontColor('#999999')
                  .margin({ top: 4 })
              }
              .layoutWeight(1)
              .alignItems(HorizontalAlign.Center)
            }
            .width('100%')
          }
          .width('100%')
          .padding(16)
          .backgroundColor(Color.White)
          .borderRadius(8)
          .shadow({
            radius: 4,
            color: 'rgba(0, 0, 0, 0.1)',
            offsetX: 0,
            offsetY: 2
          })
          .margin({ bottom: 16 })

          // ========== 客户端连接标题 ==========
          Text('客户端连接')
            .fontSize(16)
            .fontWeight(FontWeight.Bold)
            .fontColor('#333333')
            .margin({ bottom: 8 })
            .alignSelf(ItemAlign.Start)

          // ========== 连接列表 / 空状态 ==========
          if (this.connections.length === 0) {
            // 空状态提示
            Row() {
              Text('暂无客户端连接')
                .fontSize(14)
                .fontColor('#BBBBBB')
            }
            .width('100%')
            .height(80)
            .justifyContent(FlexAlign.Center)
          } else {
            // 连接列表
            List({ space: 8 }) {
              ForEach(this.connections, (item: ConnectionInfo) => {
                ListItem() {
                  this.ConnectionItem(item)
                }
              }, (item: ConnectionInfo) => item.client_id)
            }
            .width('100%')
          }
        }
        .width('100%')
        .padding(16)
      }
      .layoutWeight(1)
      .scrollBar(BarState.Auto)
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#F5F5F5')
  }

  // ========== 连接列表项组件 ==========

  /**
   * 单个客户端连接的列表项
   * 对应 Android item_connection.xml 布局
   */
  @Builder
  ConnectionItem(item: ConnectionInfo) {
    Column() {
      // 第一行：客户端 ID + 状态标签
      Row() {
        // 客户端 ID
        Text(item.client_id)
          .fontSize(15)
          .fontWeight(FontWeight.Bold)
          .fontColor('#333333')
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .layoutWeight(1)

        // 状态标签
        Text(item.status)
          .fontSize(12)
          .fontColor(Color.White)
          .backgroundColor('#4CAF50')
          .padding({ left: 8, right: 8, top: 2, bottom: 2 })
          .borderRadius(4)
      }
      .width('100%')
      .alignItems(VerticalAlign.Center)
      .margin({ bottom: 6 })

      // 第二行：订阅数 + 入站消息 + 出站消息
      Row() {
        Text(`订阅: ${item.subscriptions.length}`)
          .fontSize(12)
          .fontColor('#888888')
          .layoutWeight(1)

        Text(`入站: ${item.incoming_publish_count}`)
          .fontSize(12)
          .fontColor('#888888')
          .layoutWeight(1)

        Text(`出站: ${item.outgoing_publish_count}`)
          .fontSize(12)
          .fontColor('#888888')
          .layoutWeight(1)
      }
      .width('100%')
    }
    .width('100%')
    .padding(12)
    .backgroundColor(Color.White)
    .borderRadius(8)
    .shadow({
      radius: 2,
      color: 'rgba(0, 0, 0, 0.08)',
      offsetX: 0,
      offsetY: 1
    })
  }
}
