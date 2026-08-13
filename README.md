# DiCar Tune

面向电子设计大赛与智能车竞赛的无线调参、遥测和参数固化工作台。

DiCar Tune 通过 DCTP v1 协议连接车辆，在桌面端集中完成参数读取、RAM 调参、Flash 固化、编码器标定、最多 8 路实时波形和链路诊断。当前版本为 **0.1.2**，优先支持 Windows 桌面 App，并提供可直接体验的内置模拟器。

> 无线串口模块只负责透明传输。连接真实车辆时，车端 MCU 必须运行兼容的 DCTP 固件；仓库自带 C99 车端参考库（[firmware/dctp-device](firmware/dctp-device/README.md)），可直接移植到常见竞赛 MCU。

## 下载 Windows 0.1.2

| 版本 | 适用场景 | 下载 |
| --- | --- | --- |
| 安装版 | 日常使用，创建标准 Windows 安装 | [DiCar Tune 0.1.2 Setup](release/DiCar-Tune-0.1.2-Windows-x64-Setup.exe) |
| 便携版 | 免安装测试，直接运行可执行文件 | [DiCar Tune 0.1.2 Portable](release/DiCar-Tune-0.1.2-Windows-x64-Portable.exe) |

两个版本都包含内置模拟器，不需要额外启动后台服务。发布文件尚未进行商业代码签名，Windows 首次运行时可能显示安全提示。

## 5 分钟体验

1. 下载并启动安装版或便携版。
2. 在顶部连接栏保持“模拟器体验”。
3. 点击“连接模拟器”，等待状态变为“已连接”。
4. 从首页打开“实时调参与波形”。
5. 修改一个参数并点击“写入 RAM”，观察待固化状态。
6. 打开波形通道，体验暂停、时间窗口、游标和标记。
7. 点击“审阅并固化”，确认 RAM 与 Flash 的差异后完成模拟固化。

## 已实现功能

- 菜单式工作区与独立实时调参页面。
- 可切换的“通用 Manifest”与 DiCar 差速车任务工作区，支持受限 YAML 车型导入。
- 速度环按目标/实际/误差/输出组织参数与遥测，推荐通道只预选、确认后才订阅。
- 由设备 Manifest 驱动的数值、布尔和枚举参数控件。
- RAM、Flash、Revision 和断线未知状态分别展示。
- 编码器左右 PPR、正交倍频、只读 CPR、方向、轮径、传动比和测速过滤参数。
- 最多 8 路混合类型遥测波形，支持暂停、游标、窗口、标记和数据表。
- nanoUART-wl、HC-05 Bluetooth SPP 和通用 COM 配置。
- 自动波特率探测、串口类型显示、链路带宽保护和连接诊断。
- Owner、Tuner、Observer 本地演示权限与单车控制权提示。
- 内置 DCTP 模拟器、协议重试、CRC、会话和参数版本冲突处理。
- 车端 DCTP v1 参考库（纯 C99、零动态分配），由 Rust 权威实现和黄金向量逐字节交叉验证。

## 车型配置

顶部“车型配置”决定 App 如何把 Manifest 组织为控制环、参数任务和波形预设。它不会增加设备命令，也不会覆盖设备给出的类型、范围、可写性、RAM 或 Flash 真值。内置配置位于 `apps/dicar-desktop/src/vehicleProfiles/builtins/`；用户可从齿轮按钮导入 `.yaml`/`.yml`。

配置按区分大小写的完整 `machine_name` 精确绑定。缺失引用会显示兼容性提示并保留仍可用部分；完全不兼容时仍可通过“全部参数”或“通用 Manifest”访问设备清单。用户配置最多 16 个、合计 2 MiB，单文件最多 256 KiB；同 ID 更新必须明确确认替换，且不能覆盖内置 ID。

## 硬件兼容性

| 硬件/入口 | 当前支持 | 推荐设置 | 遥测安全上限 |
| --- | --- | --- | --- |
| nanoUART-wl | Windows 桌面真实串口 | 自动探测，优先 460800 baud | 460800 baud 时最多 8 通道 × 500 Hz |
| HC-05 | Windows Bluetooth Classic SPP 传出 COM | 先在 Windows 配对，再自动探测 | 通常 4 通道 × 50 Hz；9600 baud 时 2 通道 × 10 Hz |
| 通用串口 | Windows 桌面 COM | 选择与 MCU 一致的波特率 | 根据波特率自动限制 |
| Web Serial | 可发现并授权浏览器串口 | Chromium 系浏览器 | 真实 DCTP 会话尚未接入 |

真实硬件的接线、Windows 配对和故障排查见[用户手册](docs/user-guide.md)。

## 文档

- [用户手册：安装、接线、调参、波形与排障](docs/user-guide.md)
- [开发文档：架构、环境、测试、协议与打包](docs/development.md)
- [DCTP v1 协议设计](docs/superpowers/specs/2026-08-10-dicar-serial-collaboration-protocol-design.md)

## 当前限制

- 无线固件烧录流程尚未接入桌面 UI；当前版本聚焦参数调试和遥测。
- 纯 Web 客户端尚不能建立真实 DCTP 设备会话。
- 权限和控制权是本地演示策略，不是云端安全或分布式租约系统。
- 云账户、团队协作、远程控制、插件市场、自动 PID、AI 建议和多车并发仍属于后续版本。
- 手机、平板、macOS 和 Linux 客户端尚未发布。
- nanoUART-wl 与 HC-05 的软件适配已完成，但仍需要在用户的具体模块、固件和赛场环境中进行实体链路验证。

## 开发与验证

需要 Node.js 22 或更高、pnpm 11 和 Rust stable：

```powershell
pnpm install --frozen-lockfile
pnpm dev
```

浏览器打开 `http://127.0.0.1:5173/`。完整构建、测试、模拟器和 Windows 打包命令见[开发文档](docs/development.md)。

## 项目状态

0.1.2 是可安装、可运行模拟器、可接入 Windows COM 的首个硬件兼容版本。项目仍处于首版迭代阶段，协议和核心状态管理已有自动化测试保障，但真实车辆接入前仍应先在断电、安全架起或低功率条件下验证参数范围与控制方向。
