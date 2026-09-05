# CAN 接入设计（v1 待实现）

状态：**设计稿 + 编解码基座**。`core/slcan.py` 已提供 SLCAN ASCII 帧编解码（纯函数、带测试），
但 CAN 传输层**尚未接入** TransportManager——原因见下文"为什么先不接"。

## 目标

让带 CAN 总线的智能车（竞赛常见）也能用 DiCAR LAB 调参/看遥测，复用现有
TEL/SET/GET/CMD/ACK JSON 协议与全部上层页面。

## 方案：PC 侧 slcan 适配器 + MCU 侧 CAN↔串口桥

- PC 端通过 USB-CAN 适配器（Lawicel CAN-USB 及兼容，SLCAN ASCII 协议）收发 CAN 帧。
  `core/slcan.py` 负责帧 ⇄ ASCII 行的编解码，复用 pyserial 打开适配器串口。
- MCU 端需要一段**桥接固件**：CAN 帧 ⇄ 应用层消息。这一步必须烧进车上的 MCU，
  属于硬件阶段工作，因此本版本只交付基座不接通。

## JSON over CAN 分帧规范（约定稿）

CAN 经典帧数据场最多 8 字节，JSON 行必须分帧：

- 字节 0（控制字节）：高 4 位 = 会话序号 seq（0~15 循环），低 4 位 = 分片索引 fragment。
- 首分片 fragment=0 携带消息总长度（字节 1~2，小端）；后续分片从偏移
  `(fragment-1)*7` 继续携带最多 7 字节 payload。
- 末分片 payload 不足 7 字节以实际长度结束；接收方在 fragment 回绕或收到
  下一 seq 首分片时重置重组缓冲。
- 遥测类 TEL 建议按字段拆分成多帧发布（参考 dctp 协议手册的字段分组），
  避免单条 JSON 超过 4~5 帧。

## 为什么先不接

1. 没有 MCU 侧桥接固件之前，接通传输层无法端到端验证，交付即摆设。
2. 分帧协议一旦上车就难以变更——先在文档里定稿并留出仿真实测路径
   （可仿照 `tests/bsl_simulator.py` 写 CAN 桥模拟器）。
3. 传输层抽象已就位（串口/BLE/TCP/仿真），接入点明确：
   `core/transport.py` 新增 `connect_can(port, bitrate)` 走 slcan 编解码即可。

## 硬件阶段待办

- [ ] MCU 桥接固件（F103/F4/MSPM0 各一份示例）
- [ ] CAN 桥模拟器 + TransportManager CAN 后端 + 契约测试
- [ ] USB-CAN 适配器实测（建议 Lawicel CAN-USB 或兼容 SLCAN 固件）
