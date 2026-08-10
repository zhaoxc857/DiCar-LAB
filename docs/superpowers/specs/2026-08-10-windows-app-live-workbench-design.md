# DiCar Tune Windows App 实时调参纵向切片设计规格

- 状态：已确认，继承此前通过的产品与 DCTP 设计
- 日期：2026-08-10
- 基线：feature/dctp-v1-foundation @ 1fb7588
- 交付平台：Windows 桌面端，浏览器开发模式作为 UI 自动化宿主

## 1. 设计目标

本子项目交付第一个真正可运行、可测试、可展示的 DiCar Tune App，而不是静态原型。用户从 B 型卡片菜单主页进入 A 型实时调参工作台，App 通过 TCP 测试传输连接 dctp-sim，并沿真实 DCTP 字节流完成握手、Manifest 获取、参数读取、RAM 写入、显式固化和遥测订阅。

这一步先证明产品交互与协议闭环。下一子项目把相同 Transport 接口接到 Windows COM 和真实无线 DAP；C11 车端 SDK 也复用同一协议向量，不改变本 App 的领域模型。

## 2. 方案选择

评估过三种推进方式：

1. 先做 C11 SDK：最接近真实车辆，但短期仍看不到 App。
2. 先做纯前端静态原型：最快看到页面，但无法证明协议、参数状态和波形闭环。
3. 双轨纵向切片：App 先接已验证模拟器形成真实闭环，同时把 C11 SDK 作为紧随其后的独立计划。

采用方案 3。它让用户可见功能最早落地，又不会复制或绕过已经验证的 DCTP 基础。

## 3. 本阶段范围

### 3.1 必须完成

- Tauri 2 + React + TypeScript + Vite 的 Windows 桌面应用骨架。
- 独立 Rust dicar-app-core crate，不能依赖 Tauri UI。
- 先补齐 DCTP v1 的持久化状态闭环：PARAM_VALUE 返回 RAM 与可选 Flash 值，PARAM_COMMIT_ACK 返回 CRC32 与 Storage Generation，模拟器真实执行幂等 Commit。
- 把默认模拟器 Manifest 扩充到至少 16 个候选遥测通道，并为 PID、编码器原始计数、左右轮速、车速、误差、PWM 和故障 flags 生成随设备时间变化的确定性样本，保证 8 通道工作台不是常量假波形。
- 基于 std::net::TcpStream 的模拟器 Transport。
- DCTP HELLO、Manifest、参数全量读取、心跳、超时、断开和重新连接。
- B 型主页：项目、车辆、连接状态、固件、参数数、遥测通道数、最近固化摘要。
- A 型工作台：参数导航、类型化编辑、编码器校准、8 通道波形、底部变更栏。
- RAM 值、Flash 值、Revision 和未固化状态分别显示。
- PARAM_WRITE 的范围校验、Revision 冲突和设备最终值回显。
- PARAM_COMMIT 的审阅清单、权限门禁、成功或失败状态。
- 1 至 8 个混合 f32、i32、u32、flags32 通道订阅。
- Canvas 波形、暂停、时间窗口、游标、当前值、序号缺口和丢弃计数。
- 本地 fixture 驱动的 Owner、Tuner、Observer 角色演示；界面明确标记为本地演示权限，不冒充云端安全边界。
- 浏览器开发模式使用 MockBridge，桌面模式使用 TauriBridge；业务组件不得直接 import Tauri API。
- Rust、React 组件和 Playwright 端到端自动化测试。

### 3.2 本阶段明确不伪装完成

- Windows COM 枚举与真实串口传输。
- 真实账户、团队、WebSocket 波形转发和分布式租约。
- CMSIS-DAP/SWD/JTAG 烧录。
- 记录回放、CSV、参数方案库与 Git 导出。
- macOS、Linux、手机、平板、Web Serial、AI/PID 自动整定和多车并行。

这些仍属于完整产品目标，分别进入后续计划；主页对应入口可以可见，但必须显示真实开发状态，不能提供假数据或假成功。

## 4. 系统结构

### 4.1 Rust 工作区

新增 crates/dicar-app-core，负责：

- Transport 抽象及 TcpTransport。
- ProtocolSession：Sequence、Session ID、请求匹配、重试、心跳和状态机。
- ParameterWorkspace：Manifest、RAM/Flash 值、Revision、写入合并和 dirty 状态。
- TelemetryEngine：订阅、时间戳展开、样本缺口、环形缓冲和 UI 批次。
- Diagnostics：RTT、吞吐、解析错误、样本缺口、丢弃数和重连原因。
- AppActor：在专用线程中串行持有 Transport 与 ProtocolSession，通过有界命令/事件通道与外界通信。

新增 apps/dicar-desktop/src-tauri，职责仅为：

- 持有 AppActorHandle。
- 把 Tauri commands 转换为 CoreCommand。
- 通过一个类型化 Tauri Channel 把有序 CoreEvent 批量发给 WebView。
- 管理窗口关闭前的未固化确认。

Tauri 层不复制 DCTP 编解码，不直接解释参数 Payload。普通 Tauri Event 不承载遥测；前端通过 open_core_channel 命令注册 Channel<BridgeEvent>，命令返回后由同一 Channel 顺序传送快照、操作结果和遥测批次。

### 4.2 核心接口

Transport 是阻塞、可替换、单连接接口：

- connect(endpoint) -> TransportIdentity
- read(buffer) -> byte count
- write_all(bytes)
- close()

TcpTransport 使用 10 ms 读取超时和 1 s 写入超时，默认目标为 127.0.0.1:7100。后续 SerialTransport 复用同一接口。

AppActor 接收以下命令：

- ConnectSimulator
- Disconnect
- WriteParameter
- CommitParameters
- RevertAllPendingChanges
- SetTelemetrySubscription
- PauseTelemetry
- ResumeTelemetry
- SelectLocalAccessProfile
- Shutdown

AppActor 发出以下事件：

- SnapshotChanged
- TelemetryBatch
- OperationCompleted
- ConnectionLost
- FatalError

所有队列有固定容量；快照事件可以合并为最新值，可靠操作结果不能被遥测挤掉。

会话严格使用 DCTP v1 时序：500 ms 心跳；连续 3000 ms 无有效帧则失效；普通读写 300 ms 超时并最多重试 3 次；Manifest 分片 500 ms 并最多重试 3 次；固化 3000 ms 并最多重试 2 次。所有重试复用原 Sequence。

## 5. 前端结构

应用路由：

- /：B 型菜单主页。
- /live/:vehicleId：A 型实时调参工作台。
- /records：记录回放入口，当前阶段显示准确的开发状态。
- /parameter-sets：参数方案入口，当前阶段显示准确的开发状态。
- /diagnostics：连接诊断页，展示本阶段已实现的实时诊断。
- 其他路径：中文 NotFound 页面并提供返回主页操作。

状态边界：

- connectionStore：连接生命周期、设备身份和诊断。
- workspaceStore：Manifest、参数状态、过滤、dirty 变更和遥测选择。
- collaborationStore：本地演示身份、角色、控制租约和门禁原因。
- settingsStore：主题、窗口、波形窗口与收藏。

组件只能通过 DesktopBridge 接口调用后端。tauriBridge、mockBridge 都实现该接口，测试无需启动桌面 WebView。

DesktopBridge 只暴露 connect、disconnect、writeParameter、commitParameters、revertAll、setTelemetrySubscription、setPaused、selectAccessProfile、getSnapshot 和 subscribe。subscribe 返回解除订阅函数；Tauri 实现使用 Channel，Mock 实现使用进程内有界发布器。

## 6. B 型菜单主页

顶部固定状态条显示：

- 当前项目。
- 当前车辆。
- 传输类型与端点。
- TCP 模拟器模式显示端点且不显示波特率；后续串口传输只有在取得真实配置后才显示波特率。
- 连接、握手、READY 或断线文本状态。

主体是四张有图标和文字的功能卡：

1. 实时调参与波形。
2. 数据记录与回放。
3. 参数方案库。
4. 连接与链路诊断。

实时卡可以连接模拟器并进入工作台。诊断卡进入真实诊断页。尚未实现的记录和参数方案页明确说明当前阶段与后续计划，不显示虚假成功。

右侧或下方展示项目摘要：固件版本、Manifest CRC、参数数、可选通道数、当前 dirty 数和最近成功固化时间。

## 7. A 型实时调参工作台

目标窗口 1280 x 720 及以上使用固定三栏：

- 左栏 264 px：搜索、收藏、仅看已修改、最近调整、参数分组和数量。
- 中栏最小 420 px：参数名称、单位、描述、RAM/Flash 双值、Revision、范围、步长和类型化控件。
- 右栏最小 440 px：Canvas 波形、图例、当前值、暂停、窗口、游标和链路指标。

底部固定变更栏显示 dirty 项数量，提供撤销全部、创建本地快照的后续入口和审阅并固化。离开页面、断开或切换车辆时有 dirty 变更必须确认。

1024 至 1279 px 时，左栏进入抽屉，中栏与右栏保持双栏。小于 1024 px 时使用参数与波形两个明确标签页，底部变更栏始终可达。所有核心操作在 200% 缩放下仍能使用。

键盘操作固定为：Ctrl+K 聚焦参数搜索；Ctrl+Shift+L 连接或断开；波形获得焦点时 Space 暂停或继续；Ctrl+Z 撤销最近一次已确认 RAM 修改；M 在当前波形时间线上添加本地事件标记。快捷键不能覆盖输入框内的系统编辑行为。

## 8. 参数与编码器交互

类型映射：

- i32、u32、f32：带可见 Label 的数字输入；适合时额外提供滑杆，输入仍是精确值来源。
- bool：Switch，并同时显示开或关文本。
- enum：Select，展示中文标签和底层数值。
- read-only：只读值卡，不使用 disabled 输入伪装可编辑。
- dangerous：显示危险标记和解释，提交审阅中单独分组。

编码器面板必须明确分开：

- 左右 PPR。
- 正交倍频 1、2、4。
- 只读左右 CPR。
- 左右方向修正。
- 轮径与传动比。
- 采样周期、低通截止、跳变阈值、最大可信 RPM 和丢脉冲诊断。

PPR、倍频和 CPR 不合并为编码器线数。原始计数、轮速、车速、误差、PWM 和异常 flags 都能加入波形或当前值表。

参数写入期间每个参数最多一个在途请求；连续调整合并为最新目标值。失败时恢复设备确认值并在控件附近显示原因。Revision 冲突显示设备当前值和 Revision，由用户重新选择，不静默覆盖。

## 9. 波形与性能

Rust 侧保留原始 TelemetryBatch，并展开 u32 时间戳。每个订阅最多 8 通道，默认环形窗口 60 秒；500 Hz 时每通道最多 30,000 点。

WebView 每 16 至 33 ms 收到一个批次，而不是每个样本一个事件。Canvas 使用每像素 min/max 桶降采样；记录源与绘图源概念分离，为后续无损记录留出接口。

波形必须提供：

- 明确图例，通道不仅靠颜色，还使用实线、虚线或点线。
- 当前值和单位。
- 暂停或继续。
- 1、5、10、30、60 秒窗口。
- 键盘可操作游标与时间/数值读数。
- 数据表或文本摘要作为可访问性替代。
- 空数据、加载、断线冻结和解析失败状态。

UI 更新上限 30 Hz。Canvas 绘制不得阻塞参数 ACK 展示或输入反馈。

## 10. 视觉系统

风格为现代暗色工程仪表台：高密度、低装饰、清晰分层。禁止霓虹泛光、无意义玻璃模糊和持续背景动画。

核心颜色：

| 语义 | 颜色 |
| --- | --- |
| 背景 | #071018 |
| 主表面 | #0B1620 |
| 次表面 | #101F2B |
| 边框 | #263746 |
| 主文字 | #F2F7FA |
| 次文字 | #A8BAC7 |
| 交互/焦点 | #38BDF8 |
| 成功 | #34D399 |
| 警告 | #FBBF24 |
| 危险 | #FB7185 |

采用 shadcn New York 组件风格、6 px 基础圆角和 4/8 px 间距节奏。正文采用系统中文字体栈，数据、时间和 Revision 使用等宽 tabular figures。Phosphor 线性图标是唯一结构图标来源。

所有普通文字对比度至少 4.5:1；图形和大文字至少 3:1。状态必须同时包含图标或文本，不只靠颜色。键盘焦点使用 2 px 蓝青色环。

动效仅表达因果，持续 150 至 220 ms，只使用 opacity 和 transform，并尊重 prefers-reduced-motion。

## 11. 权限与安全表达

本阶段有三个本地演示身份：

- Owner：观察、调参、固化。
- Tuner：观察、调参，无固化权限。
- Observer：仅观察。

只有活动控制租约持有者且具备调参权限时可以写 RAM。只有具备固化权限且租约处于 active 时可以 Commit。禁用控件旁必须显示文字原因。

该 fixture 只验证前端门禁和状态表达；页面醒目标记本地演示权限。真正账户、审计、波形转发和 3 秒分布式租约由协作服务阶段实现。

## 12. 异常与恢复

- TCP 断线：立即停止写命令，冻结波形，dirty 状态改为设备状态未知。
- Session 失效：停止重试并重新 HELLO；旧写入不自动回放。
- CRC 或解析失败：累计诊断，解析器等待下一个分隔符恢复。
- 请求超时：普通命令按 DCTP 规则有限重试；按钮显示进行中并防止重复提交。
- Manifest 变化：丢弃旧描述缓存并重新读全部值。
- Commit 失败：保留 RAM 值和 dirty 状态，不显示已固化。
- 离开或关闭：dirty 变化必须明确选择留在页面、放弃本地工作区显示或断开但保留未知状态。

## 13. 测试与验收

Rust 单元与集成测试：

- PARAM_VALUE 的 RAM/Flash 双值、非持久化空值、Commit Generation 单次递增、重复 Commit 幂等和存储失败保持旧 Flash 值。
- 默认模拟器至少 16 个候选通道，任意 8 通道可订阅，连续样本随设备时间变化且同一时间输入可重复。
- TcpTransport 的连接、EOF、读取超时、写入和关闭。
- ProtocolSession 的 HELLO、Manifest、参数读取、心跳、断开和重连。
- 参数范围、类型、Revision 冲突、每参数单在途和最新值合并。
- Commit CRC、权限拒绝、成功与失败 dirty 状态。
- 8 通道订阅、时间戳回绕、序号缺口、60 秒环形上限和 UI 批次。
- 与真实 dctp-sim TCP 服务的完整闭环。

React/Vitest 测试：

- B 主页状态和四入口。
- B 到 A 路由并保留车辆。
- 三栏工作台与窄窗口适配。
- 五种参数类型、RAM/Flash 双值和失败恢复。
- 编码器 PPR、倍频、CPR 分离。
- 8 通道上限、暂停、游标和数据表。
- Owner、Tuner、Observer 门禁与文字原因。
- dirty 离开确认、断线冻结和错误 live region。
- Ctrl+K、Ctrl+Shift+L、Space、Ctrl+Z 和 M 快捷键的作用域与输入框保护。

Playwright 流程：

1. 1280 x 720 打开主页。
2. 连接模拟器并进入工作台。
3. 修改 PID 和编码器参数。
4. 查看 ACK 后的 RAM/Flash 差异。
5. 订阅 8 路波形并暂停、移动游标。
6. 以 Tuner 身份确认不能固化。
7. 切换 Owner，审阅并固化。
8. 模拟断线，确认冻结与未知状态。

质量门禁：

- Rust fmt、check、Clippy warnings-as-errors 和 workspace tests。
- pnpm lint、typecheck、Vitest、production build 和 Playwright。
- axe 无严重或关键违规。
- 1280 x 720、1024 x 768、768 x 1024 和 200% 缩放快照。
- DCTP golden vectors 必须保持不变。

## 14. 后续接口承诺

后续 SerialTransport、Recorder、ParameterSetService、CollaborationGateway、FlashCoordinator、Web Serial 和多车调度都接在本阶段稳定接口上。任何后续客户端都复用 dctp-protocol 的 Schema、错误码和黄金向量，不能在 TypeScript 中定义第二套不兼容协议。
