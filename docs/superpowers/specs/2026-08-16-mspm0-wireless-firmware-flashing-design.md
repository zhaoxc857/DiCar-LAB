# MSPM0G3507 无线固件烧录设计规格

- 状态：已批准（用户授权后续采用推荐默认）
- 日期：2026-08-16
- 首版目标：立创·天猛星 MSPM0G3507 + HC-05/nanoUART 透明串口 + TI ROM BSL

## 1. 目标与边界

DiCar Tune 在 Windows Tauri App 内完成经过签名的固件包检查、安全停机、DCTP 会话退出、TI ROM BSL 擦写与校验、应用重启、DCTP 重连和结果审计。HC-05/nanoUART 只提供透明 UART；实际 Flash 操作由 MSPM0G3507 ROM BSL 执行。

首版只对 `LCKFB-TMX-MSPM0G3507` 做真实适配。STM32 F1/F4 只通过 target adapter 接口保留后续扩展点，不实现、不展示为可用。浏览器/Web Serial、模拟器和非 Tauri 环境只能使用显式 Mock，不得访问真实烧录命令。

TI 官方资料确认 MSPM0G3507 ROM BSL 支持 UART Flash 擦写、CRC 校验、256 位密码保护、应用软件请求进入 BSL，默认 UART 为 9600 8N1；天猛星的 BSL UART 为 PA10/PA11，板载 BSL/RST 按键可用于人工恢复：

- <https://www.ti.com/lit/pdf/SLAU887>
- <https://www.ti.com/lit/ds/symlink/mspm0g3507.pdf>
- <https://wiki.lckfb.com/zh-hans/web-tool/mspm0-web-flasher/introduce.html>

## 2. 安全模型

1. 只有已连接真实串口、Owner 权限、活动租约、零未固化参数且设备声明 `PREPARE_FLASH` 能力时允许开始。
2. 固件必须是 `.dicarfw` v1 签名包；Rust 后端验证目标板、长度、SHA-256 和 Ed25519 发布签名。前端显示结果但不参与信任判断。
3. 每台设备使用独立 32 字节 BSL 密码。密码按 DCTP `device_id` 存入 Windows Credential Manager，不出现在前端、日志、固件包、命令行参数或仓库。
4. 发布私钥离线保存且不进入仓库。独立预配/打包 CLI 从标准输入或显式外部路径读取敏感材料；测试只使用公开标记的非生产测试种子。
5. 烧录获得全局独占锁。`PREPARE_FLASH_ACK` 后禁止普通连接、参数、遥测和窗口关闭；擦除开始后不提供伪安全取消，只允许继续、重试或刷回恢复包。
6. App 不宣称设备端 A/B。主机保存上一份已验证签名包作为恢复包；首次烧录前必须由预配工具为设备登记恢复包。

## 3. `.dicarfw` v1 格式

文件按以下顺序编码，所有整数为 little-endian：

```text
magic[8]          = "DICARFW\0"
format_version    = u16 = 1
manifest_len      = u32, 1..8192
image_len         = u32, 1024..131072
manifest_json     = manifest_len 个 UTF-8 字节
image             = image_len 个字节
signature         = 64 字节 Ed25519
```

manifest JSON 必须拒绝未知字段并包含：

```json
{
  "schemaVersion": 1,
  "releaseId": "UUID",
  "target": "lckfb-tmx-mspm0g3507",
  "mcu": "MSPM0G3507",
  "firmwareVersion": [0, 3, 0],
  "imageBase": 0,
  "imageLength": 123456,
  "imageSha256": "64 lowercase hex chars",
  "signingKeyId": "16 lowercase hex chars"
}
```

签名输入为 `"DiCarFW-v1\0" || header_without_magic || manifest_json || image` 的原始字节。解析器先检查总长度和各段上限，再解析 JSON、计算 SHA-256、查找预配记录允许的 `signingKeyId`，最后验签。包内版本可以升级或降级；降级必须在最终确认页明确标红，不能静默阻止恢复包。

## 4. DCTP v1 安全切换

沿用已保留的 `PREPARE_FLASH (0x50)` / `PREPARE_FLASH_ACK (0x51)`，不改变帧头、消息 ID 或其他消息；为这两种消息补齐 v1 payload 和黄金向量。

`PREPARE_FLASH` payload：

```text
schema_version:u8 = 1
operation_id:[u8;16]
target_id:u32 = 1  // LCKFB-TMX-MSPM0G3507
firmware_version:[u16;3]
image_len:u32
image_sha256:[u8;32]
```

`PREPARE_FLASH_ACK` payload：

```text
schema_version:u8 = 1
operation_id:[u8;16]
bootloader_protocol:u8 = 1  // TI_MSPM0_ROM_BSL_UART
entry_delay_ms:u16 = 250
initial_baud:u32 = 9600
```

设备只有在项目回调已安全关闭电机/高功率输出、停止遥测并锁定写操作后才返回 ACK。C 库的 `write` 合约只保证完整接受帧，因此平台适配层必须等待 UART TX complete，再调用一次性 pending-transition API 触发 `SYSRST + BOOTLOADERENTRY`。重复请求走既有幂等缓存，不能重复执行安全停机回调。

## 5. 主机架构与状态机

新增 `dicar-firmware-flash` Rust crate，职责分离为：

- package：严格解析/签名/验签 `.dicarfw`；
- target：`FirmwareTargetAdapter` 与首个 `Mspm0g3507TmxAdapter`；
- bsl：TI UART ROM BSL packet、ACK/core response、超时、擦除、分块写入、standalone CRC、start application；
- orchestrator：与串口无关的升级状态机、恢复包和故障分类；
- tool：离线签名、包生成、每设备凭据/恢复包预配。

Tauri 新增独立 `FirmwareFlashServiceState`，类似 AI 服务但需要协调 `AppState` 的设备独占锁。React 新增独立 `FirmwareFlashPlatform`，不得把长时间烧录方法塞进普通 `DesktopBridge`。

状态机固定为：

```text
unavailable -> selecting -> validating -> ready -> confirming
-> preparing -> switchingTransport -> unlocking -> erasing
-> programming -> verifying -> restarting -> reconnecting -> succeeded

任一关键阶段失败 -> recoveryRequired -> retrying | rollingBack
-> reconnecting -> succeeded | recoveryRequired
```

开始后按顺序执行：封存前端录制、获取升级锁、发送 DCTP `PREPARE_FLASH`、让 Core 释放串口、等待 250 ms、以 9600 8N1 打开相同 COM、连接 ROM BSL、读取设备信息、解锁、按镜像范围擦除、按设备 buffer 分块写入、比较设备 CRC、启动应用、关闭 BSL 串口、用原 endpoint 重连 DCTP，并核对相同 device ID、目标固件版本和完整 Manifest。

## 6. HC-05 与恢复规则

HC-05 MCU 侧 UART 固定配置为 9600 8N1；同一物理链路上的 DCTP 首版也使用 9600，不能在 ROM BSL 改速后假定 HC-05 会同步改速。HC-05 TX 接 PA11/BSL_RX，HC-05 RX 接 PA10/BSL_TX，模块在 MCU reset 期间持续供电；板载 CH340 与 HC-05 不得同时驱动 PA10/PA11。

软件切换失败或镜像不可启动时，向导显示天猛星人工恢复步骤：保持 HC-05 连接，按住 BSL，按下并松开 RST，松开 BSL，随后点击重试/刷回恢复包。窗口关闭保护保持有效，直到新固件重连成功或操作者明确确认设备仍处于恢复状态。

## 7. 验收

- 协议：Rust/C 对新增 payload 逐字节一致；能力位、幂等、错误路径和黄金向量受测试锁定。
- 包：截断、超长、未知字段、错误 target、SHA 漂移、未知 key、坏签名全部在任何设备命令前拒绝。
- BSL：使用确定性 fake serial 覆盖 ACK 错误、超时、密码失败、分块边界、CRC 不匹配、断线和恢复包路径。
- Tauri：升级锁阻止竞争连接/写入/关闭；凭据不进入 DTO/错误文本；成功后必须完成 DCTP 重连核验。
- 前端：文件检查、风险确认、进度、不可取消临界区、恢复/回滚和键盘可访问性均有 Vitest/Playwright 覆盖。
- 硬件：只有天猛星 + HC-05 实际完成升级、断电中断、手动 BSL/RST 恢复和恢复包回刷后，文档才可标记“实板验证”。硬件未就绪时软件交付必须明确写“未做实板验证”。

## 8. 明确延期

- STM32 F1/F4 target adapter；
- MSPM0G3519/天巧星；
- 设备端自动 A/B；
- 云端固件分发、账户、远程烧录；
- HC-05 运行时 AT 模式切速；
- 浏览器真实烧录。
