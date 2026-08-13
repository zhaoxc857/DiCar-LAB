# DiCar Tune 模拟器 PID 闭环增强设计

日期：2026-08-14

## 1. 目标

让 Rust `dctp-sim` 与 Web 预览 `MockBridge` 在无硬件环境中提供真实可观察的速度闭环，使 AI 自动调参可以完成“写入 RAM 增益 → 运行阶跃实验 → 采集响应指标 → 比较下一轮”的端到端流程。

本设计不改变 DCTP v1 wire 格式，不修改黄金向量，不把演示车辆模型耦合进通用 C99 设备库，也不改变“AI 只写 RAM、固化必须人工确认”的安全边界。

## 2. 已确认的根因

当前链路同时存在以下缺口，必须联动修复：

1. Rust 模拟器 manifest 只有 `pid.kp`，缺少 Ki、Kd 和可写目标速度参数。
2. 内置车型 YAML 没有 `target_parameter`，因此 AI 向导会把速度环判定为不可自动调参；Mock 模式同样受影响。
3. C 交叉验证 shim 的静态 manifest 与 Rust 默认 manifest 逐字段、逐字节比较，Rust 单边新增参数会破坏 CRC 和行为测试。
4. Rust 与 Mock 的速度遥测均不响应 PID/目标参数。
5. Mock 只在连接或切换订阅时预生成有限样本，向导的实时等待窗口没有持续遥测。
6. Mock 时间戳固定增长 2 ms，不尊重订阅采样率；100 Hz 实验的指标时间轴会缩短五倍。
7. Mock 把目标速度当成持久参数；自动实验可能把激励目标混入 Flash 审阅。
8. 向导没有统一清理路径，会遗留实验目标值和实验订阅；中止最多延迟到一次 3 秒保持结束后才生效。
9. Mock 写参数不检查有限值和数值范围，与 Rust 设备校验语义不一致。

## 3. 范围与兼容性

### 3.1 纳入范围

- Rust 与 TypeScript 各实现一个隔离、可单测的速度闭环模型。
- 默认 Rust manifest、Mock fixtures、内置车型 YAML 与 C 测试 shim 的稳定名称保持一致。
- Mock 增加生命周期受控的实时遥测调度器，同时保留确定性手动推进入口。
- AI 向导保存并恢复实验前目标与遥测设置，在所有退出路径执行清理。
- 扩展测试与开发文档，证明模拟闭环可用且既有协议契约未回归。

### 3.2 不纳入范围

- 不修改 `firmware/dctp-device` 的通用协议或车辆控制逻辑。生产 C99 SDK 继续使用嵌入方提供的参数表和遥测回调。
- 不要求 Rust 与 JavaScript 输出逐位一致；要求模型公式、参数影响、限幅和收敛趋势一致。
- 不新增协议消息、字段、通道 ID 或遥测类型。
- 不调用真实 DeepSeek API 作为自动化测试条件。

## 4. 参数与车型契约

四个闭环参数使用以下稳定 machine name：

| ID | machine name | 类型 | 默认值 | 范围 | 标志 |
|---:|---|---|---:|---:|---|
| 1 | `pid.kp` | f32 | 1.2 | 0–20 | writable, persistent |
| 2 | `pid.speed.ki` | f32 | 0.08 | 0–5 | writable, persistent |
| 3 | `pid.speed.kd` | f32 | 0.002 | 0–1 | writable, persistent |
| 4 | `control.target_speed_mps` | f32 | 0 | 0–8 m/s | writable, dangerous, non-persistent |

参数 ID 与名称在 Rust、Mock 和 C shim 中完全一致。参数范围选择与当前 Mock fixtures 对齐，并将 Rust 原先宽泛的 Kp 范围收紧为演示控制器的有效范围。

目标速度是实验激励，不是待调增益。它可由向导写入 RAM，但 `persistedValue` 为 `null`，不产生 dirty 状态，不进入 commit plan。`dangerous` 标志继续触发 UI 安全提示；向导的增益白名单排除仅作用于待整定参数，不阻止目标激励。

内置 `dicar-diff-drive.yaml` 的速度环增加：

- `target_parameter: control.target_speed_mps`
- `Ki: pid.speed.ki`
- `Kd: pid.speed.kd`

现有目标/反馈/误差/PWM 遥测名称与通道 ID 保持不变。

## 5. 速度闭环模型

### 5.1 模型边界

Rust 和 TypeScript 各有一个只负责车辆动力学的 `SpeedLoopModel`。协议设备和 Bridge 只向模型提供当前参数、目标和时间推进，并读取模型快照；模型不知道 DCTP frame、React store、订阅版本或 Flash。

模型状态至少包括：

- 当前车辆速度；
- 积分项；
- 上一次速度与滤波后的测量导数；
- 当前归一化电机输出；
- 已推进到的模拟时刻。

### 5.2 控制与车辆方程

模型使用固定 2 ms 内部步长，避免遥测采样率改变控制环行为。每一步执行：

1. `error = target - speed`。
2. 使用速度变化率构造测量值导数，避免目标阶跃造成 derivative kick；对导数做一阶低通。
3. `u_raw = Kp * error + Ki * integral - Kd * d(speed)/dt`。
4. 将输出夹在 `[-1, 1]`，并用条件积分抑制 wind-up；积分本身也有硬限幅。
5. 当目标为零时复位积分项，使向导默认 800 ms 静息阶段得到可重复基线。
6. 使用一阶惯性车辆模型将归一化输出映射为速度，并把最终速度夹在物理范围内。

常量（车辆时间常数、最大速度、导数滤波常数、积分限幅）在两端取同样数值。测试比较允许小容差，不比较浮点位模式。

### 5.3 动态遥测

以下通道从同一个模型快照生成：

- `drive.target_speed_mps`
- `drive.speed_mps`
- `drive.speed_error_mps`
- `drive.left_wheel_speed_mps`
- `drive.right_wheel_speed_mps`
- `motor.left_pwm`
- `motor.right_pwm`

左右轮速度在车辆速度基础上加入小幅、确定性的静态差异；PWM 从归一化输出映射到 0–1000 permille。编码器计数、故障、抖动、电池、转向、运行时间和自定义通道继续使用既有确定性生成方式。

## 6. 时间与批次语义

### 6.1 Rust

`SimDevice::tick(now_ms)` 根据订阅周期计算到期样本。对需要丢弃的过旧样本，模型仍推进到首个实际发送样本的时刻，保证状态对应真实经过的模拟时间；随后对每个发送样本依次推进并读取快照。

批次的 sequence、dropped counter、容量限制、base timestamp、`dt_us` 和时间戳回绕规则保持原样。

### 6.2 Mock

Mock 时间戳增量改为 `1_000_000 / sampleRateHz`，不再固定为 2 ms。`advanceTelemetry(sampleCount)` 仍同步、确定性地推进指定样本数，供单测和大批量波形测试使用。

当满足以下全部条件时，Mock 启动实时调度器：

- phase 为 ready；
- 存在 active subscription；
- 未暂停；
- 至少有一个 Bridge listener。

调度器以短周期唤醒，根据单调时钟计算应补发样本数，并分批调用同一推进路径。暂停、断线、最后一个 listener 取消订阅或订阅变化时停止/重建调度，避免 timer 泄漏与测试进程悬挂。WebSerialBridge 的模拟器入口继承相同行为；真实浏览器串口验证路径不会启动模拟器调度。

## 7. AI 向导实验隔离

开始实验前保存：

- 目标参数当前 RAM 值；
- 当前 desired subscription；
- 当前暂停状态。

实验订阅只包含目标和反馈，采样率为 100 Hz。实验结束后，无论原因是成功、轮数上限、看门狗、API 失败、写入失败或人工中止，统一执行：

1. 恢复原目标 RAM 值；
2. 恢复原订阅；若原先无订阅，则调用 Bridge 的显式清除订阅操作，同时清空 desired/active subscription 并保持暂停；
3. 保留引擎写回的最佳增益 RAM 值；
4. 若清理失败，在最终状态消息中明确报告，不能把运行本身伪报为完整成功。

等待实现为短分片的可中止等待。人工中止后不必等满整个保持窗口；实验返回无指标时，引擎先检查 abort 状态，再决定是 `aborted` 还是“实验无效”。向导同时用 `AbortController` 将取消信号传给自动调参引擎和 `DeepSeekClient`，因此等待 AI 回复时也能立即取消；客户端必须区分用户取消与内部 60 秒超时。

配置校验增加目标静息值和阶跃值的有限性及 manifest 范围检查，避免启动后才收到设备拒绝。

现有 Bridge 只有“暂停但保留 desired subscription”的操作，无法恢复“实验前无订阅”。本次增加 `clearTelemetrySubscription()`，Core/Tauri/Mock 均实现该接口。连接设备时它复用既有 `TELEMETRY_STOP` 请求，不增加新 wire 消息；清除成功后 desired 与 active subscription 均为 `null`，paused 为 `true`。

## 8. Mock 参数语义

Mock 写参数与设备端保持关键一致性：

- 校验权限、writable、类型、有限值和 numeric 范围；
- persistent 参数按 RAM 与 persisted 值计算 dirty；
- non-persistent 参数始终 `persistedValue: null`、`dirty: false`；
- commit 和 revert 只处理可持久且 dirty 的参数；
- 重连保留已接受的 RAM 参数，与“断线不回滚 RAM”约束一致；车辆动态状态在新模拟器会话开始时复位，避免旧速度泄漏到新演示。

## 9. 测试策略

### 9.1 Rust 模型与设备

- 零目标保持静止，输出有限。
- 目标阶跃后速度上升，默认增益在三秒内形成稳定、可度量响应。
- Kp/Ki/Kd 变化会改变响应指标；输出、积分和速度始终受限。
- 回零后积分复位，重复实验基线稳定。
- 100 Hz 和 500 Hz 遥测采样不会改变相同墙钟时间下的模型趋势。
- 批量追赶按时间顺序推进，sequence/drop/timestamp 既有测试继续通过。
- 新参数描述、默认值、flags 和类型被 manifest contract 测试锁定。

### 9.2 TypeScript 模型与 Mock

- 模型测试覆盖与 Rust 相同的行为性质。
- Mock 目标/PID 写入会影响速度、误差和 PWM 遥测。
- 100 Hz 时间戳间隔为 10,000 us。
- fake timers 证明有 listener 时持续产样，暂停/断线/取消订阅后停止，恢复后不重复启动。
- non-persistent 目标写入不增加 dirtyCount，commit 不固化目标。
- NaN、Infinity、越界和类型错误被拒绝。

### 9.3 车型、向导与跨语言契约

- 内置车型在 Mock 和 Rust 默认 manifest 上均解析为可自动调参速度环，并暴露 Kp/Ki/Kd。
- 向导失败/中止路径恢复目标和订阅；引擎把中止实验识别为 aborted。
- Core、TauriBridge 和 Mock 的显式清除订阅操作都清空 desired/active 状态，且暂停后的清除不会错误恢复旧订阅。
- AI 客户端收到外部取消信号时终止 fetch 并报告取消，内部超时仍报告超时。
- C shim manifest 与 Rust default manifest 继续逐字节相等，CRC 一致。
- 六个黄金向量检查保持不变。

## 10. 验证门禁

实现完成后运行：

- 前端 `lint`、`typecheck`、全部单测、生产构建和 e2e；
- Rust `fmt --check`、workspace `clippy -D warnings`、workspace 全测试；
- `generate_vectors --check`；
- 对最终 diff 做一次安全约束复核，确认目标仍是 RAM-only、AI 不自动固化、协议 wire 未变。
