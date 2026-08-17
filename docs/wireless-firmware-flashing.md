# MSPM0G3507 无线固件升级

DiCar Tune 已实现面向立创开发板天猛星 MSPM0G3507 的首版无线固件升级软件链：
Windows 桌面端通过 HC-05 或 nanoUART-wl 的透明串口先发送 DCTP 安全切换请求，
设备确认电机停机并释放串口后，再由桌面端使用 TI ROM BSL 写入经过 Ed25519
签名验证的 `.dicarfw` 包。

> 当前状态：协议、签名包、桌面服务、恢复流程、界面和主机自动化已经实现；
> 天猛星实板、HC-05 空中链路、NONMAIN 配置和断电恢复尚未验证。现有
> `release/` 里的 0.2.0 安装包也没有重新构建，因此不能把本页的软件结果当成
> 已发布或已通过硬件验收的功能。

## HC-05 能做什么

HC-05 只提供 Bluetooth Classic SPP 透明串口，不会自己解析、保存或烧写 MCU
固件。它可以承载烧录字节流，但必须同时满足：

- 目标 MCU 自带或已经集成可用的 Bootloader；首版只接入 TI MSPM0 ROM BSL。
- 车端 DCTP 固件实现 `PREPARE_FLASH`，能先安全停机、发出 ACK、等待 UART
  完整发送，再进入 Bootloader。
- HC-05、应用 UART 和 ROM BSL 都配置为 9600 baud、8N1；首版不在运行时发送
  HC-05 AT 命令切速。
- Windows 使用 HC-05 的传出 COM，升级期间没有串口助手或下载器占用同一端口。

因此答案是“可以作为无线传输通道”，不是“HC-05 本身就是烧录器”。

## 启动升级前的硬性条件

桌面端只有在以下条件全部成立时才允许进入 ROM BSL：

1. 连接的是已就绪的真实串口设备，硬件类型为 HC-05 或 nanoUART-wl，速率为
   9600 baud。
2. 当前本地身份为 Owner 且控制权有效，设备声明 `PREPARE_FLASH` 能力，RAM 与
   Flash 之间没有待固化参数。
3. `.dicarfw` 目标是 `lckfb-tmx-mspm0g3507`，镜像为 1–128 KiB，SHA-256、
   Ed25519 签名和设备专属可信公钥全部验证通过。
4. Windows Credential Manager 中已有该设备的 32 字节 BSL 密码，主机上已有
   同一设备的已签名恢复包。
5. 降级时已经显式确认；普通升级也必须勾选车辆安全和恢复准备确认。

升级开始后，App 独占该设备：普通连接、参数写入、断开和窗口关闭不会与烧录
并发。设备接受 `PREPARE_FLASH` 之后不可普通取消，因为此时应用固件可能已经
不再运行。

## 桌面端操作

1. 让驱动轮离地，限制电机电源并准备人工断电。
2. 用 9600 baud 连接已完成配置的天猛星设备，确认状态为“已就绪”且待固化参数
   为零。
3. 在设备抽屉打开“无线固件烧录”，选择 `.dicarfw` 文件。
4. 核对目标板、版本、镜像长度、签名 Key ID 和 SHA-256 摘要；如为降级，单独
   确认降级。
5. 确认安全停机与恢复准备，启动升级。进度依次为准备、切换串口、解锁、擦除、
   写入、校验、启动和重连。
6. 只有 App 重新连接到相同设备 ID、读到目标版本并保存新恢复包后，才显示成功。

## 中断后的恢复

擦除开始后断电或断链不会自动变成“成功”。App 会保持设备升级锁并显示
`recoveryRequired`：

1. 断开电机等高功率负载，保持 MCU、无线模块和电脑端链路供电稳定。
2. 按天猛星与 TI BSL 手册使用 BSL 引脚/复位操作，让 MCU 再次进入 ROM BSL。
3. 保持同一个传出 COM 可用，在向导中选择“重试候选固件”或“刷回恢复固件”。
4. 如果无线链路无法恢复，改用有线 BSL/SWD 工具；不要反复让电机带载上电。

主机恢复不是设备端 A/B 固件槽。首版依赖 ROM BSL、每设备密码和主机保存的已
签名恢复包；恢复过程中仍可能需要人工 BSL/RST 操作。

## 离线签名与设备配置

查看工具帮助：

```powershell
cargo run -p dicar-firmware-flash --bin dicar-firmware-tool -- --help
```

生成签名包：

```powershell
cargo run -p dicar-firmware-flash --bin dicar-firmware-tool -- package `
  --release-id <UUID> --version <MAJOR.MINOR.PATCH> `
  --signing-key-id <16位小写十六进制> --image <app.bin> `
  --key <离线Ed25519私钥文件> --output <release.dicarfw>
```

工具拒绝覆盖已存在的输出文件。发布私钥不应复制到车辆电脑，也不得提交到仓库。

导入单个设备的可信公钥、恢复包与 BSL 密码：

```powershell
cargo run -p dicar-firmware-flash --bin dicar-firmware-tool -- provision-record `
  --device-id <32位小写十六进制> `
  --signing-key-id <16位小写十六进制> `
  --public-key <release.pub> `
  --recovery-package <known-good.dicarfw> `
  --store-dir "$env:LOCALAPPDATA\DiCar\firmware"
```

命令启动后从标准输入粘贴 32 字节原始值或 64 位小写十六进制密码并回车。密码
不支持命令行 `--password`；DiCar Tune 不负责把 BSL 密码写进 MCU NONMAIN。
NONMAIN 的 BSL 启用、接口、密码和安全策略必须先按 TI 文档在实体板上核对。

## 当前目标范围与硬件门禁

| 目标 | 软件状态 | 发布门禁 |
| --- | --- | --- |
| 天猛星 MSPM0G3507 | 首版适配已实现 | ARM 交叉编译、正常升级、擦除/写入中断、BSL+RST 恢复、回滚均未实板验证 |
| MSPM0G3519 | 后续目标 | 尚未实现目标适配与实板验证 |
| STM32F1/F4 | 后续目标 | 需要独立 Bootloader/协议适配，不能复用 TI ROM BSL |

TI ROM BSL 行为以 [MSPM0 Bootloader User's Guide](https://www.ti.com/lit/ug/slau887a/slau887a.pdf)
为准；天猛星车端接入代码位于
[`firmware/targets/lckfb-tmx-mspm0g3507/`](../firmware/targets/lckfb-tmx-mspm0g3507/README.md)。
