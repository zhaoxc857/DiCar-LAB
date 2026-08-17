# DiCar Tune 开发文档

本文说明 DiCar Tune 0.2.0 的代码边界、开发环境、质量门禁和 Windows 打包流程。使用说明见[用户手册](user-guide.md)。

当前 `release/` 内 0.2.0 安装版与便携版已于 2026-08-17 按第 7、9 节完整流程重新构建，包含兼容首页、精准控制台以及天猛星 MSPM0G3507 无线烧录页面和 Tauri 固件服务；版本号保持 0.2.0。

## 1. 架构

```mermaid
flowchart LR
    UI["React 工作区"] --> Bridge["DesktopBridge 接口"]
    UI --> AiPlatform["AiPlatform"]
    UI --> FirmwarePlatform["FirmwareFlashPlatform"]
    UI --> Recorder["RecordingController"]
    Bridge --> Tauri["Tauri Commands / Events"]
    AiPlatform --> AiService["Tauri AI Service"]
    AiService --> Credential["Windows Credential Manager"]
    AiService --> DeepSeek["固定 DeepSeek HTTPS"]
    FirmwarePlatform --> FirmwareService["Tauri Firmware Service"]
    FirmwareService --> Credential
    FirmwareService --> RomBsl["TI ROM BSL / COM"]
    Recorder --> IndexedDB["原始批次 IndexedDB"]
    Tauri --> Actor["dicar-app-core AppActor"]
    Actor --> Session["DCTP ProtocolSession"]
    Session --> Serial["Windows COM / HC-05 / nanoUART-wl"]
    Session --> Sim["dctp-sim TCP 模拟器"]
```

主要边界：

- React 只消费快照和桥接事件，不直接打开串口或构造 DCTP 帧。
- `AppShell` 固定提供概览、实时调试、波形记录、诊断四个顶部路由；首页另以四张真实功能卡呈现实时调试、记录回放、参数方案和链路诊断。`ConnectionDrawer` 仍是唯一连接表单并只调用既有 `DesktopBridge`。
- 车辆 YAML 只把 Manifest 的精确 `machine_name` 解析成任务；resolver 输出稳定的 `paramId`/`channelId`，不修改设备 DTO。
- Tauri 层负责类型化命令、事件转发、串口发现和内置模拟器生命周期。
- 独立 `AiPlatform` 把 React 与 Tauri AI command 隔离；Rust AI service 独占凭据、HTTP、限制和取消，Key 不进入前端状态。
- 独立 `FirmwareFlashPlatform` 把 React 向导与 Tauri 固件服务隔离；服务独占升级
  锁、签名信任、每设备 BSL 凭据、ROM BSL 串口和恢复包，普通 Bridge 不传输固件。
- `RecordingController` 在唯一 Bridge 事件入口先复制原始遥测批次，再交给实时绘图 store；IndexedDB 和回放不修改 DCTP wire 或设备状态。
- `dicar-app-core` 负责单线程 AppActor、会话、权限策略、参数工作区、链路预算和遥测缓冲。
- `dctp-protocol` 只负责 DCTP v1 编解码和 payload 模型，不执行 IO。
- `dctp-sim` 是确定性的 TCP 设备模拟器，用于协议、参数、遥测和桌面集成测试。
- `firmware/dctp-device` 是车端 C99 参考实现：只依赖注入的串口、Flash 和时钟回调，不含具体芯片外设代码；`crates/dctp-device-c` 在 `cargo test` 时把它编译进来，用黄金向量和 Rust 协议栈做逐字节交叉验证（需要本机 C 编译器，Windows 上为 MSVC）。移植方式见 [firmware/dctp-device/README.md](../firmware/dctp-device/README.md)。

## 2. 仓库结构

```text
apps/
  dicar-desktop/
    src/                 React + TypeScript UI、bridge、stores
    src-tauri/           Tauri 2 Windows 壳与 Rust commands
    e2e/                 Playwright 关键流程
crates/
  dctp-protocol/         DCTP v1 wire codec 与消息模型
  dctp-sim/              确定性模拟设备和 TCP server
  dctp-device-c/         车端 C 参考库的 Rust 交叉验证 harness
  dicar-app-core/        会话、AppActor、参数与遥测核心
  dicar-firmware-flash/  签名包、TI ROM BSL、凭据、恢复与离线工具
firmware/
  dctp-device/           车端 DCTP v1 参考库（C99）与移植指南
  targets/               具体 MCU/开发板的安全切换适配
docs/
  user-guide.md          使用者手册
  development.md         本文
  superpowers/           设计规格与实施计划
release/                 当前 Windows 发布文件（主工作区）
```

前端通过 `DesktopBridge` 抽象选择 Tauri、Web Serial 或 Mock 实现。纯 Web 预览的真实 DCTP 会话尚未完成，不能把浏览器授权端口误报为已连接设备。

### 车辆配置 schema v1

内置配置放在 `apps/dicar-desktop/src/vehicleProfiles/builtins/`，由 Vite 作为 raw text 打包并在模块初始化时严格解析。最小完整示例：

```yaml
schema_version: 1
vehicle: { id: my-diff-car, display_name: 我的差速车, type: 双轮差速, order: 50 }
control_loops:
  - id: speed
    label: 速度环
    target_parameter: control.target_speed_mps
    gains: { Kp: pid.kp, Ki: pid.speed.ki, Kd: pid.speed.kd }
    telemetry:
      target: drive.target_speed_mps
      feedback: drive.speed_mps
      error: drive.speed_error_mps
      outputs: [motor.left_pwm, motor.right_pwm]
    recommended_channels: [drive.target_speed_mps, drive.speed_mps, drive.speed_error_mps]
parameter_sections:
  - { id: encoder, label: 编码器, parameters: [encoder.left.ppr, encoder.right.ppr] }
scope_presets:
  - { id: drive, label: 驱动总览, channels: [drive.speed_mps, motor.left_pwm, motor.right_pwm] }
```

约束：ID 仅允许小写 ASCII、数字、`-`、`_`；拒绝 YAML 锚点、别名和 merge key；未知字段报错；每类最多 32 项、每项最多 64 个引用。单文件 256 KiB，用户配置最多 16 个且总计 2 MiB。绑定区分大小写且不使用显示名称。解析问题为导入失败；Manifest 兼容问题按 error/warning 记录并保留有效任务，所有参数始终可从通用工作区访问。

## 3. 开发环境

通用要求：

- Node.js 22 或更高（`.node-version` 固定当前推荐版本）
- pnpm 11（仓库声明 `pnpm@11.16.0`）
- Rust stable
- Git

Windows 桌面开发和打包还需要：

- Visual Studio 2022 C++ Build Tools
- Windows SDK
- MSVC Rust target：`stable-x86_64-pc-windows-msvc`
- Microsoft Edge WebView2 Runtime

安装 Rust target：

```powershell
rustup target add x86_64-pc-windows-msvc
```

## 4. 安装依赖与运行 Web

在仓库根目录执行：

```powershell
pnpm install --frozen-lockfile
pnpm dev
```

浏览器打开：

```text
http://127.0.0.1:5173/
```

Web 预览默认提供确定性模拟体验。支持 Web Serial 的 Chromium 浏览器可以授权和识别 USB 串口，但真实 DCTP 会话仍应使用 Windows 桌面 App。
Web 预览不启用 AI，也不接受或保存 DeepSeek Key；只有 Tauri Windows 桌面壳会创建可用的 `AiPlatform`。

### 精准控制台前端结构

- 顶部设备状态芯片只显示连接真值，点击后打开 `ConnectionDrawer`；抽屉分为连接、硬件指南和偏好，导航后窄屏抽屉会关闭。
- `settingsStore` schema v4 在既有串口与 `aiModel` 设置外增加 `workbenchMode: "standard" | "track"`。该字段只影响 CSS grid 与密度，不进入 `DesktopBridge`，切换模式不发送订阅、暂停、写参数或固化命令。
- `WorkbenchLayout` 始终按导航、编辑器、波形的同一 DOM 顺序渲染；标准/赛道模式不会卸载编辑器或波形，因此参数草稿、录制状态和实时缓冲保持不变。
- `TelemetryStrip` 只读取已解析控制环、RAM 参数、绘图环形缓冲和 `AppSnapshot`；缺失字段显示“—”，不推算实测 RX 速率或健康评分。
- `RecordingLibrary` 复用现有 `RecordingController`，由 `/records` 独立页面承载；回放仍使用独立只读缓冲。
- 参数方案没有第二套页面或 store：`/live?panel=snapshots` 打开工作台现有 `SnapshotManagerDialog`，关闭时只移除 `panel` 查询参数并保留其他参数；旧 `/parameter-sets` 使用 replace 重定向到该地址。
- `FirmwareFlashEntry` 只在已就绪的 HC-05/nanoUART-wl 9600 baud 真实串口上启用，
  打开独立 `FirmwareFlashWizard`；非 Tauri 环境使用显式 unavailable 平台。

## 5. 运行模拟器

查看 CLI：

```powershell
cargo run -p dctp-sim -- --help
```

在固定 TCP 地址运行独立模拟器：

```powershell
cargo run -p dctp-sim -- --listen 127.0.0.1:7100
```

Windows 发布版使用 Tauri 内嵌的 `SimulatorServer`，监听系统分配的本地端口，不依赖上述独立进程。

Rust `dctp-sim` 与 Web `MockBridge` 都实现相同语义的简化速度闭环：固定 2 ms 内部控制步长、PID 控制器和一阶车辆惯性。以下稳定名称在内置车型中构成完整调参契约：

- `pid.kp`、`pid.speed.ki`、`pid.speed.kd`：可写、可持久化，但自动调参只修改 RAM，固化仍需人工确认。
- `control.target_speed_mps`：可写、dangerous、非持久化，仅作为实验激励，不进入 Flash dirty/commit 集合。
- `drive.target_speed_mps`、`drive.speed_mps`、`drive.speed_error_mps`、左右轮速和左右 PWM：由闭环模型动态生成。

Mock 遥测按请求采样率生成设备时间戳，并仅在设备 ready、订阅 active、未暂停且存在监听者时运行实时调度器。AI 向导结束、失败或中止时会恢复实验前目标值与订阅/暂停状态；如果实验前没有订阅，则明确清除实验订阅。该清除操作复用 DCTP v1 `TELEMETRY_STOP`，没有增加 wire message。

## 6. 运行 Windows 桌面 App

确保当前终端已加载 Visual Studio MSVC 环境，然后从仓库根目录运行：

```powershell
pnpm --filter @dicar/desktop exec tauri dev
```

Tauri 会执行配置中的 `beforeDevCommand`，启动 Vite，并创建 Rust AppActor 与内置模拟器。真实串口连接由 Tauri command 将前端 `Endpoint` 映射为核心类型：

```text
Simulator { address }
Serial { portName, baudRate, hardwareProfile }
```

`hardwareProfile` 当前可取：

- `nanoUartWl`
- `hc05BluetoothSpp`
- `genericSerial`

### 安全 AI 桌面通道

`apps/dicar-desktop/src-tauri/src/ai_service.rs` 是与设备 `AppState` 分离的服务。
它固定请求 `https://api.deepseek.com/chat/completions`，禁止重定向，连接/总超时
分别为 10/60 秒，并以流方式把响应限制在 1 MiB。模型名只接受 1–64 个安全
ASCII 字符。每个请求使用前端生成的 UUID 和 `CancellationToken` 注册；
`ai_cancel` 会真正终止 Rust Future，RAII guard 负责所有正常、错误和 Future
drop 路径的请求表清理。

凭据适配使用精确 `keyring 3.6.3` 的 `windows-native` 后端，服务/用户为
`com.dicar.tune` / `deepseek-api-key`。Tauri 注册以下命令：

- `ai_credential_status`
- `ai_set_api_key`
- `ai_clear_api_key`
- `ai_complete`
- `ai_cancel`

前端 `settingsStore` schema v4 保存串口偏好、`aiModel` 和纯前端
`workbenchMode`，迁移在 Zustand hydration 前直接清除旧 `aiBaseUrl`/`aiApiKey`。
React 只消费 `AiPlatform`，不直接 import
Tauri invoke。Rust 测试用内存凭据替身和本地 HTTP server，自动化不访问真实
DeepSeek。

### MSPM0G3507 无线固件升级

`apps/dicar-desktop/src-tauri/src/firmware_service.rs` 在升级前获取 AppState 的
RAII 独占锁，验证真实串口、Owner/控制权、零 dirty、设备能力、目标/版本、签名
公钥、设备 BSL 密码和恢复包，再通过 DCTP `PREPARE_FLASH` 请求车端安全停机。
ACK 后 Core 释放串口且不发送普通 `SESSION_CLOSE`，固件服务等待切换并以 9600
8N1 执行 TI ROM BSL 的连接、解锁、擦除、分块写入、CRC 校验和启动。重连必须
同时匹配原设备 ID 与目标固件版本。

可复用边界位于 `crates/dicar-firmware-flash`：

- `.dicarfw` v1 使用严格有界 Manifest、镜像 SHA-256 与 Ed25519 签名；
- BSL 密码按设备存入 Windows Credential Manager，不返回前端；
- 恢复包按设备原子替换，擦除后的错误保留升级锁，只允许重试或回滚；
- `dicar-firmware-tool` 是离线签名/配置工具，密码只走 stdin，输出文件拒绝覆盖。

车端通用 DCTP 库仍不含 TI 寄存器代码。天猛星适配在
`firmware/targets/lckfb-tmx-mspm0g3507/`：先 `safe_stop`，再发送 ACK，等待
`uart_tx_complete`，最后调用 DriverLib 的 BOOTLOADER_ENTRY 复位入口。MSP
工程需选择 `__MSPM0G3507__` 并加入目标目录两个 `.c`。本机没有 TI Arm Clang
或 Arm GCC；真实 SDK 交叉编译、NONMAIN 和实板流程仍是发布门禁。

完整操作和恢复说明见[无线固件升级指南](wireless-firmware-flashing.md)。

### 原始波形记录与回放

`useBridgeSubscription` 是唯一事件扇出。它先调用 `RecordingController.acceptEvent`
（同步深拷贝事件并进入单一 Promise 队列），再更新连接和 60 秒绘图 store。
记录仓储使用原生 IndexedDB `dicar-tune-recordings` v1：

- `recordings` 以记录 UUID 保存元数据；
- `recordingChunks` 用 `[recordingId, chunkIndex]` 复合主键保存完整 `UiTelemetryBatch`，并按 `recordingId` 建索引；
- 时间跨度达到 1 秒或累计 4096 点时写块；单次 5 分钟，库上限 20 条 / 256 MiB；
- 所有写入串行；写失败删除整条活动记录，启动时删除所有非 complete 记录；
- 导入先做 schema v1 全量验证，再在单个事务内写元数据和全部块；清理旧记录也与导入处于同一事务。

JSON 保存元数据和原始批次；CSV 是按采样时刻展开的宽表并处理公式注入。
导出和回放用引用计数保护记录不被容量清理。回放把完整记录加载进独立
`TelemetryRingBuffer`，只向 `WaveformCanvas` 传显式 `viewportEndUs`；它不读取
实时 store，也不调用 `DesktopBridge`。记录管理由 `/records` 的
`RecordingLibrary` 呈现；工作台只保留开始/停止录制和进入该页面的链接。

## 7. 质量门禁

### 前端

```powershell
pnpm lint
pnpm typecheck
pnpm --filter @dicar/desktop test --run
pnpm build
pnpm test:e2e
```

当前前端基线为 44 个 Vitest 文件 / 194 个测试、11 个 Playwright 场景。除原有
Bridge、调参、记录、AI、波形和可访问性覆盖外，Vitest 还验证固件平台命令、
入口资格、降级确认、关键阶段不可取消和恢复操作。Playwright 仍使用 Mock，
不访问真实串口、DeepSeek 或 Bootloader。

### Rust workspace

通用开发环境：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Windows 发布前使用 MSVC toolchain 复跑：

```powershell
cargo +stable-x86_64-pc-windows-msvc fmt --all -- --check
cargo +stable-x86_64-pc-windows-msvc clippy --workspace --all-targets -- -D warnings
cargo +stable-x86_64-pc-windows-msvc test --workspace --all-targets
```

关键约束包括：

- 任何可靠请求均保持同一 Sequence 重试。
- 错误 Session 和精确 3000 ms 失效边界会停止写入。
- 参数 ACK 是 RAM 与 Revision 的权威来源。
- Commit 以冻结的参数集合和 canonical CRC 原子确认。
- 串口失败或 DCTP 握手失败不会残留“已连接”身份。
- 链路预算在发送 `TELEMETRY_SUBSCRIBE` 前由核心验证。

## 8. DCTP 黄金向量

无线烧录协议增加 `PREPARE_FLASH` 请求/ACK 两个向量，生成器与交叉测试现在要求
共八个 DCTP v1 二进制向量：

```powershell
cargo run -p dctp-sim --bin generate_vectors -- --check
```

成功输出：

```text
DCTP v1 vectors match
```

仅在有意修改 wire contract 时才运行不带 `--check` 的生成器。协议变更必须同时审查：

- 帧布局和 COBS/CRC；
- 消息类型与 flags；
- 参数 value、state、write、commit；
- Manifest 分片和协商载荷上限；
- 遥测批次、序列缺口和 dropped counter；
- 旧黄金向量是否构成兼容性承诺。

## 9. Windows 打包

版本号必须同时更新：

- `apps/dicar-desktop/package.json`
- `apps/dicar-desktop/src-tauri/Cargo.toml`
- `apps/dicar-desktop/src-tauri/tauri.conf.json`
- `Cargo.lock` 中的 `dicar-desktop` 条目

在 Visual Studio x64 开发环境中执行：

```powershell
pnpm --filter @dicar/desktop tauri:build
```

若 Windows Smart App Control 误拦 Cargo 生成的未签名 build script，不要关闭系统策略；
可只改变可再生中间产物的哈希后重试，最终应用仍使用优化 release profile：

```powershell
$env:CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_DEBUG = "1"
pnpm --filter @dicar/desktop tauri:build
```

默认输出：

```text
target/release/dicar-desktop.exe
target/release/bundle/nsis/DiCar Tune_<version>_x64-setup.exe
```

发布前应执行：

1. 完整前端、Rust、Playwright 和黄金向量门禁。
2. 将安装版和便携版复制到主仓库 `release/`，使用统一名称。
3. 计算 SHA-256。
4. 实际启动便携版，确认进程保持运行且内置模拟器拥有监听端口。
5. 只终止本次测试启动的精确 PID。
6. 在新版本验证后清理旧发布文件和任务专用构建目录。

## 10. 修改硬件适配时的约束

新增硬件配置时应保持以下边界：

- 在 Rust 与 TypeScript 中使用相同的 `SerialHardwareProfile` 序列化名称。
- 波特率必须经过明确 allow-list，不把任意数字直接传入串口层。
- 自动探测只调用连接流程，不写参数、不固化 Flash、不启动遥测。
- 只有完整 DCTP 握手和设备加载成功后才能保存端口配置。
- 失败尝试必须保持 Disconnected，不能留下 `transportIdentity`。
- 每种硬件配置必须定义保守的 `TelemetryBudget`。
- UI 限制用于指导使用者，核心限制才是不可绕过的安全边界。
- 不根据商品宣传直接承诺距离、速率或抗干扰性能；实体链路必须单独验证。
- HC-05 使用 Windows Bluetooth Classic SPP 传出 COM，不把它描述为 Web Bluetooth。
- 无线烧录仅允许独立 Firmware service 在升级锁内执行；不得通过普通
  `DesktopBridge` 发送镜像，也不得把参数固化文案复用于固件升级。

## 11. 贡献建议

- 一次变更只解决一个清晰问题，避免同时重写协议、核心和 UI。
- 修改行为前先添加能失败的聚焦测试，再实现最小修复。
- 公共类型和 wire layout 的修改视为潜在 breaking change。
- 保持 UI 只消费 bridge/store，不在组件中直接调用 Tauri API。
- 不提交 `target/`、临时测试目录或已被替代的发布二进制。
- 每个里程碑结束时检查旧发布物、临时输出、失效文档和被替代资产；先解析并核实精确路径，优先移入回收站或采用其他可恢复清理方式，不使用宽泛递归删除。
- 无线固件烧录软件首版完成后，下一门禁是天猛星与 nanoUART-wl/HC-05 实体
  验证；在硬件验收前不得描述为可发布支持。MSPM0G3519 与 STM32 适配仍为后续。
- 提交前运行与变更范围匹配的聚焦测试，并在最终阶段运行完整门禁。

返回[项目 README](../README.md)或查看[用户手册](user-guide.md)。
