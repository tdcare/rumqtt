/**
 * MQTT 客户端 ArkUI 页面示例
 *
 * 展示如何在 OHOS 设备上使用 rumqttc 进行 MQTT 通信：
 * 1. 连接/断开 Broker
 * 2. 订阅/取消订阅主题
 * 3. 发布消息
 * 4. 实时查看收到的消息和日志
 */

import { MqttClient, MqttQos, MqttEvent, getRumqttcClientVersion } from './MqttClient';

const MAX_LOG_COUNT = 50;
const MAX_MESSAGE_COUNT = 100;

@Entry
@Component
struct MqttExamplePage {
  // ======================== 状态 ========================
  @State isConnected: boolean = false;
  @State messages: Array<string> = [];
  @State logMessages: Array<string> = [];
  @State brokerUrl: string = 'tcp://127.0.0.1:1883';
  @State clientId: string = 'ohos-rumqttc-001';
  @State publishTopic: string = 'test/ohos/greeting';
  @State publishPayload: string = 'Hello from OHOS!';
  @State subscribeTopic: string = 'test/ohos/#';

  private client: MqttClient | null = null;

  // ======================== 生命周期 ========================
  aboutToAppear() {
    this.addLog(`rumqttc 版本: ${getRumqttcClientVersion()}`);
  }

  aboutToDisappear() {
    this.disconnectFromBroker();
  }

  // ======================== 业务方法 ========================
  private addLog(msg: string): void {
    const now = new Date();
    const ts = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}:${now.getSeconds().toString().padStart(2, '0')}`;
    const entry = `[${ts}] ${msg}`;
    this.logMessages = [entry, ...this.logMessages].slice(0, MAX_LOG_COUNT);
  }

  private connectToBroker(): void {
    if (this.client !== null) {
      this.addLog('客户端已存在，请先断开');
      return;
    }

    this.addLog(`正在连接 ${this.brokerUrl} ...`);

    try {
      this.client = new MqttClient({
        brokerUrl: this.brokerUrl,
        clientId: this.clientId,
        keepAliveSecs: 30,
        cleanSession: true,
      });

      this.client.onEvent((event: MqttEvent) => {
        switch (event.type) {
          case 'connected':
            this.isConnected = true;
            this.addLog('已成功连接到 Broker');
            break;

          case 'message': {
            const text = `${event.topic}: ${event.payload}`;
            this.messages = [text, ...this.messages].slice(0, MAX_MESSAGE_COUNT);
            this.addLog(`收到消息 - 主题: ${event.topic}`);
            break;
          }

          case 'disconnected':
            this.isConnected = false;
            this.addLog('连接已断开');
            break;

          case 'error':
            this.addLog(`错误: ${event.error}`);
            break;
        }
      });

      this.client.startEventLoop(100);
      this.addLog('事件轮询已启动');
    } catch (e) {
      this.addLog(`连接失败: ${e}`);
      this.client = null;
    }
  }

  private disconnectFromBroker(): void {
    if (this.client === null) {
      return;
    }
    try {
      this.client.disconnect();
      this.addLog('已主动断开连接');
    } catch (e) {
      this.addLog(`断开连接异常: ${e}`);
    }
    this.client = null;
    this.isConnected = false;
  }

  // ======================== UI ========================
  build() {
    Column() {
      // ---------- 标题栏 ----------
      Row() {
        Text('MQTT 客户端示例')
          .fontSize(22)
          .fontWeight(FontWeight.Bold)
          .fontColor('#FFFFFF')
      }
      .width('100%')
      .height(56)
      .backgroundColor('#4A90D9')
      .justifyContent(FlexAlign.Center)
      .alignItems(VerticalAlign.Center)

      // ---------- 可滚动内容 ----------
      Scroll() {
        Column() {
          // ===== 连接配置 =====
          this.SectionTitle('连接配置')

          Column() {
            Row() {
              Text('Broker 地址')
                .fontSize(14)
                .fontColor('#666666')
                .width(90)
              TextInput({ text: this.brokerUrl, placeholder: 'tcp://host:port' })
                .onChange((value: string) => { this.brokerUrl = value; })
                .layoutWeight(1)
                .height(40)
                .borderRadius(6)
                .backgroundColor('#F5F5F5')
                .enabled(!this.isConnected)
            }
            .width('100%')
            .margin({ bottom: 8 })

            Row() {
              Text('客户端 ID')
                .fontSize(14)
                .fontColor('#666666')
                .width(90)
              TextInput({ text: this.clientId, placeholder: '客户端唯一标识' })
                .onChange((value: string) => { this.clientId = value; })
                .layoutWeight(1)
                .height(40)
                .borderRadius(6)
                .backgroundColor('#F5F5F5')
                .enabled(!this.isConnected)
            }
            .width('100%')
            .margin({ bottom: 12 })

            // 连接 / 断开按钮
            Button(this.isConnected ? '断开连接' : '连接 Broker')
              .width('100%')
              .height(44)
              .borderRadius(8)
              .fontSize(16)
              .fontWeight(FontWeight.Medium)
              .fontColor('#FFFFFF')
              .backgroundColor(this.isConnected ? '#E74C3C' : '#27AE60')
              .onClick(() => {
                if (this.isConnected) {
                  this.disconnectFromBroker();
                } else {
                  this.connectToBroker();
                }
              })
          }
          .padding(12)
          .margin({ left: 12, right: 12, bottom: 12 })
          .borderRadius(10)
          .backgroundColor('#FFFFFF')

          // ===== 订阅区域 =====
          this.SectionTitle('订阅管理')

          Column() {
            Row() {
              TextInput({ text: this.subscribeTopic, placeholder: '订阅主题' })
                .onChange((value: string) => { this.subscribeTopic = value; })
                .layoutWeight(1)
                .height(40)
                .borderRadius(6)
                .backgroundColor('#F5F5F5')
            }
            .width('100%')
            .margin({ bottom: 10 })

            Row() {
              Button('订阅')
                .layoutWeight(1)
                .height(40)
                .borderRadius(8)
                .fontSize(15)
                .fontColor('#FFFFFF')
                .backgroundColor('#3498DB')
                .enabled(this.isConnected)
                .opacity(this.isConnected ? 1.0 : 0.5)
                .onClick(() => {
                  if (this.client !== null) {
                    const ok = this.client.subscribe(this.subscribeTopic, MqttQos.AtLeastOnce);
                    this.addLog(`订阅 '${this.subscribeTopic}' ${ok ? '成功' : '失败'}`);
                  }
                })

              Blank().width(12)

              Button('取消订阅')
                .layoutWeight(1)
                .height(40)
                .borderRadius(8)
                .fontSize(15)
                .fontColor('#FFFFFF')
                .backgroundColor('#95A5A6')
                .enabled(this.isConnected)
                .opacity(this.isConnected ? 1.0 : 0.5)
                .onClick(() => {
                  if (this.client !== null) {
                    const ok = this.client.unsubscribe(this.subscribeTopic);
                    this.addLog(`取消订阅 '${this.subscribeTopic}' ${ok ? '成功' : '失败'}`);
                  }
                })
            }
            .width('100%')
          }
          .padding(12)
          .margin({ left: 12, right: 12, bottom: 12 })
          .borderRadius(10)
          .backgroundColor('#FFFFFF')

          // ===== 发布区域 =====
          this.SectionTitle('发布消息')

          Column() {
            Row() {
              Text('主题')
                .fontSize(14)
                .fontColor('#666666')
                .width(50)
              TextInput({ text: this.publishTopic, placeholder: '发布主题' })
                .onChange((value: string) => { this.publishTopic = value; })
                .layoutWeight(1)
                .height(40)
                .borderRadius(6)
                .backgroundColor('#F5F5F5')
            }
            .width('100%')
            .margin({ bottom: 8 })

            Row() {
              Text('内容')
                .fontSize(14)
                .fontColor('#666666')
                .width(50)
              TextInput({ text: this.publishPayload, placeholder: '消息内容' })
                .onChange((value: string) => { this.publishPayload = value; })
                .layoutWeight(1)
                .height(40)
                .borderRadius(6)
                .backgroundColor('#F5F5F5')
            }
            .width('100%')
            .margin({ bottom: 10 })

            Button('发布')
              .width('100%')
              .height(44)
              .borderRadius(8)
              .fontSize(16)
              .fontWeight(FontWeight.Medium)
              .fontColor('#FFFFFF')
              .backgroundColor('#E67E22')
              .enabled(this.isConnected)
              .opacity(this.isConnected ? 1.0 : 0.5)
              .onClick(() => {
                if (this.client !== null) {
                  const ok = this.client.publish(
                    this.publishTopic,
                    this.publishPayload,
                    MqttQos.AtLeastOnce,
                    false
                  );
                  this.addLog(`发布到 '${this.publishTopic}' ${ok ? '成功' : '失败'}`);
                }
              })
          }
          .padding(12)
          .margin({ left: 12, right: 12, bottom: 12 })
          .borderRadius(10)
          .backgroundColor('#FFFFFF')

          // ===== 收到的消息 =====
          this.SectionTitle(`收到的消息 (${this.messages.length})`)

          Column() {
            if (this.messages.length === 0) {
              Text('暂无消息')
                .fontSize(14)
                .fontColor('#BBBBBB')
                .textAlign(TextAlign.Center)
                .width('100%')
                .padding(20)
            } else {
              List() {
                ForEach(this.messages, (msg: string, index: number) => {
                  ListItem() {
                    Text(msg)
                      .fontSize(13)
                      .fontColor('#333333')
                      .width('100%')
                      .padding({ left: 8, right: 8, top: 6, bottom: 6 })
                      .backgroundColor(index % 2 === 0 ? '#FAFAFA' : '#FFFFFF')
                  }
                }, (msg: string, index: number) => `msg_${index}_${msg}`)
              }
              .width('100%')
              .height(180)
              .divider({ strokeWidth: 0.5, color: '#EEEEEE' })
            }
          }
          .padding(12)
          .margin({ left: 12, right: 12, bottom: 12 })
          .borderRadius(10)
          .backgroundColor('#FFFFFF')

          // ===== 日志区域 =====
          this.SectionTitle(`日志 (${this.logMessages.length})`)

          Column() {
            if (this.logMessages.length === 0) {
              Text('暂无日志')
                .fontSize(14)
                .fontColor('#BBBBBB')
                .textAlign(TextAlign.Center)
                .width('100%')
                .padding(20)
            } else {
              List() {
                ForEach(this.logMessages, (log: string, index: number) => {
                  ListItem() {
                    Text(log)
                      .fontSize(12)
                      .fontColor('#888888')
                      .fontFamily('monospace')
                      .width('100%')
                      .padding({ left: 8, right: 8, top: 4, bottom: 4 })
                  }
                }, (log: string, index: number) => `log_${index}_${log}`)
              }
              .width('100%')
              .height(200)
              .divider({ strokeWidth: 0.5, color: '#F0F0F0' })
            }

            // 清除日志按钮
            Button('清除日志')
              .width('100%')
              .height(36)
              .borderRadius(6)
              .fontSize(13)
              .fontColor('#999999')
              .backgroundColor('#F0F0F0')
              .margin({ top: 8 })
              .onClick(() => {
                this.logMessages = [];
              })
          }
          .padding(12)
          .margin({ left: 12, right: 12, bottom: 24 })
          .borderRadius(10)
          .backgroundColor('#FFFFFF')
        }
        .width('100%')
      }
      .layoutWeight(1)
      .scrollBar(BarState.Auto)
      .edgeEffect(EdgeEffect.Spring)
    }
    .width('100%')
    .height('100%')
    .backgroundColor('#F0F2F5')
  }

  // ======================== 自定义构建器 ========================
  @Builder
  SectionTitle(title: string) {
    Text(title)
      .fontSize(15)
      .fontWeight(FontWeight.Medium)
      .fontColor('#333333')
      .margin({ left: 16, top: 4, bottom: 8 })
  }
}
