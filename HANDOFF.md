# DiCar Tune 开发交接（2026-08-17）

给新会话/新开发者的现状快照。读完本文 + 按需查阅引用文件，即可继续开发。

## 0. 无线固件升级开发快照（2026-08-17）

- 已通过规格评审并在 `codex/release-0.2.0` 实现天猛星 MSPM0G3507 首版软件链：
  DCTP `PREPARE_FLASH`、签名 `.dicarfw`、TI ROM BSL、每设备凭据、恢复包、
  AppState 独占锁、Tauri 命令和 React 向导。
- HC-05/nanoUART-wl 在首版只作为 9600 8N1 透明链路；HC-05 本身不是烧录器。
  设备 ACK 后串口从 DCTP Core 交给 ROM BSL，擦除后的失败只能重试或回滚。
- 通用 C 库已支持可选 `prepare_flash` 回调；天猛星目标适配保证先安全停机、ACK
  完整发送、再一次性 BOOTLOADER_ENTRY 复位。MSPM0 SDK 2.11.00.07 API 已按
  本地 SDK 核对，但本机无 TI Arm Clang/Arm GCC，实板交叉编译尚未验证。
- 当前自动化证据：44 个 Vitest 文件 / 194 项测试、Playwright 11/11；新增 Rust
  协议、Core、包、BSL、凭据、恢复、Tauri 服务和 C 适配测试均通过；Clippy
  `-D warnings`、lint、typecheck/build 已通过。
- 8 个黄金向量已生成，Rust 生成器与 C 端逐字节复现测试均通过。
- `release/` 0.2.0 安装版/便携版已于 2026-08-17 重建并包含无线烧录页面与后台。
  天猛星+HC-05/nanoUART-wl 的正常
  升级、擦除/写入中断、BSL+RST 恢复和回滚四项全部未做硬件验收，不得声称已支持。
- 权威资料：
  [无线烧录规格](docs/superpowers/specs/2026-08-16-mspm0-wireless-firmware-flashing-design.md)、
  [实施计划](docs/superpowers/plans/2026-08-16-mspm0-wireless-firmware-flashing.md)、
  [操作与恢复指南](docs/wireless-firmware-flashing.md)。

- 当前实现分支：`codex/release-0.2.0`，从已验证基线 `3c556f0` 创建。规格提交 `b14495d`；安全 AI 提交 `938a101`、`354d019`；波形记录/回放提交 `443f4ce`、`8b43046`、`d3a699b`、`896144e`、`2a19f82`；精准控制台 UI 设计/计划提交 `42b5687`、`38d58e9`，实施与文档提交 `ebacc2a`、`b23a941`、`07a7d34`、`184b4c0`、`6e90766`、`0239e76`、`94ba3dd`、`6d9cc02`、`9580fe6`；旧首页/新工作台兼容提交 `b726327`。
- 0.2.0 已完成门禁、NSIS/便携版构建、精确 PID/回环端口冒烟和 SHA-256 校验。发布文件位于 `release/`，校验值见 `release/SHA256SUMS.txt`。
- 0.2.0 安装版与便携版已于 2026-08-17 再次构建，包含无线固件烧录页面、Tauri 固件服务、DCTP `PREPARE_FLASH` 与 8 个黄金向量；版本号保持 0.2.0。
- 权威规格：[docs/superpowers/specs/2026-08-10-dicar-serial-collaboration-protocol-design.md](docs/superpowers/specs/2026-08-10-dicar-serial-collaboration-protocol-design.md)（DCTP v1 协议与分阶段计划）。
- 开发文档：[docs/development.md](docs/development.md)（架构、环境、门禁、打包）；用户手册：[docs/user-guide.md](docs/user-guide.md)。

## 1. 后续开发边界（用户 2026-08-14 更新，勿扩展范围）

后续工作只允许以下四类：

1. **无线固件烧录**：软件首版已按评审规格实现；下一步是补齐黄金向量并完成
   天猛星硬件门禁。不得在实板验收前重建或宣传发布支持。
2. **nanoUART-wl / HC-05 实体环境验证**：软件适配已完成；只有用户明确确认硬件准备好后才开始实测、资格验证和对应修复。
3. **完善现有功能与 UI**：允许改进当前调参、遥测、AI、记录回放、诊断和易用性，但不借机加入新的平台或云端产品范围。
4. **及时清理过期文件**：每个里程碑审计旧发布物、临时输出、失效文档和被替代资产；只删除已核实目标，优先采用可恢复操作。精准控制台阶段已删除旧占位页和旧连接栏，临时视觉审计用例与产物也已清理。

明确不做：参数方案导入、云账户/团队协作/远程控制、插件市场、多车并发、浏览器真实 DCTP/Web AI，以及 macOS、Linux、手机和平板客户端。除上述四类外，不再推荐或实施其他缺失功能。

## 2. 本阶段完成的功能（0.2.0 软件范围）

### 车端 C 参考库（`8177a93`）
- [firmware/dctp-device](firmware/dctp-device/README.md)：纯 C99、零动态分配的 DCTP v1 设备栈（会话/Manifest 流式分片/参数校验链/canonical CRC 固化/A/B 双槽/遥测批次/日志/32 项幂等缓存）。PREPARE_FLASH 未实现（与模拟器一致，返回 UNKNOWN_MESSAGE）。
- [crates/dctp-device-c](crates/dctp-device-c)：`cargo test` 时用 cc 编译 C 源并交叉验证——6 个黄金向量逐字节比对 + 20 个行为场景（Rust 协议栈驱动 C 设备）。**C 库任何改动都被这层锁住。**

### 参数方案与固化记录（`57dfd4d`，规格 §14 首版）
- [src/tuning/snapshots.ts](apps/dicar-desktop/src/tuning/snapshots.ts) 纯逻辑（捕获/按稳定 ID diff/导出 JSON/持久化解析）；[tuningSnapshotStore](apps/dicar-desktop/src/stores/tuningSnapshotStore.ts)（localStorage，64 个/1 MiB 限额）；[SnapshotManagerDialog](apps/dicar-desktop/src/components/workbench/SnapshotManagerDialog.tsx) UI。
- 应用规则（§12.3）：缺失/类型变化/越界/只读/状态未知只列出、不自动写；固化成功自动生成带 storage generation 的 commit 记录（钩子在 [CommitReviewDialog](apps/dicar-desktop/src/components/workbench/CommitReviewDialog.tsx)）。

### AI 自动调参（`cb095f0`，自动 N 轮到收敛）
- 循环：阶跃实验 → 本地指标（[metrics.ts](apps/dicar-desktop/src/tuning/metrics.ts)：上升/超调/整定/稳态误差/振荡）→ DeepSeek 决策（[aiClient.ts](apps/dicar-desktop/src/ai/aiClient.ts)，OpenAI 兼容，temperature 0，强制 JSON）→ 限幅写 RAM。引擎在 [autoTune.ts](apps/dicar-desktop/src/tuning/autoTune.ts)（依赖注入，13 个单测覆盖收敛/截断/看门狗/中止/坏输出）。
- **本地不可绕过的护栏**：增益白名单（dangerous 排除）、Manifest 范围 clamp、单轮步长 ≤ 量程 20%、看门狗（超调 >80% 或振荡 >6 → 回滚本地评分最佳轮并终止）、只写 RAM、结束必写回最佳轮。收敛与最佳轮均以本地判定为准，不信任 AI 自评。
- 入口：工作台 header"AI 调参"（[AutoTuneWizard.tsx](apps/dicar-desktop/src/components/workbench/AutoTuneWizard.tsx)）。要求车型 YAML 控制环声明 `target_parameter`（可写数值参数）+ feedback 遥测通道。

### 安全 AI 桌面通道（`938a101`、`354d019`）
- Rust/Tauri 固定请求官方 DeepSeek completions HTTPS，禁止重定向，连接/总超时 10/60 秒，响应上限 1 MiB；UUID + `CancellationToken` 使前端 Abort 真正终止 Rust 请求。
- `keyring 3.6.3` `windows-native` 把 Key 存入 Windows Credential Manager，服务/用户为 `com.dicar.tune` / `deepseek-api-key`。Key 不返回前端、不写日志、不进入错误文本。
- 五个命令已注册：`ai_credential_status`、`ai_set_api_key`、`ai_clear_api_key`、`ai_complete`、`ai_cancel`。React 只依赖独立 `AiPlatform`。
- `settingsStore` 当前为 v4：保存串口偏好、`aiModel` 和纯前端 `workbenchMode`，hydration 前删除旧 `aiBaseUrl`/`aiApiKey`。浏览器、Web Serial 和非 Tauri Mock 明确不可用，也不显示 Key 输入。

### 无硬件 PID 闭环模拟（`77181d4`–`db88b58`）
- Rust `dctp-sim` 与前端 `MockBridge` 都采用固定 2 ms 控制步长、PID + 一阶惯性模型；目标、实际速度、误差、左右轮速和左右 PWM 会随参数动态响应。两端共享行为语义，不要求逐位浮点一致。
- 清单标准名已统一：`pid.kp`、`pid.speed.ki`、`pid.speed.kd`、`control.target_speed_mps`。目标参数 writable + dangerous，但不 persistent；自动实验不会产生 Flash dirty。C 侧仅同步测试 shim 静态 Manifest，通用 C99 库未加入车辆模型。
- 内置 `dicar-diff-drive.yaml` 已声明目标参数和 Kp/Ki/Kd，因此 Mock 与 Rust 模拟器都满足 AI 向导启动条件。
- Mock 按请求采样率生成时间戳并在有监听者时实时发流；暂停、断开、清除订阅或最后一个监听者取消后停止定时器。
- 向导会校验实验目标范围，取消 50 ms 内可打断等待并穿透到 AI 请求；成功、失败、看门狗或中止都在 `finally` 恢复实验前目标与订阅/暂停状态。无原订阅时经 Core/Tauri/Bridge 的明确清除接口复用 `TELEMETRY_STOP`。
- DCTP v1 wire 格式、消息 ID、遥测通道 ID 与六个黄金向量均未改变。

### 波形记录、导入导出与回放（`443f4ce`–`2a19f82`）
- 唯一 Bridge 事件入口先把完整 `UiTelemetryBatch` 深拷贝到串行记录控制器，再交给实时绘图 store；1 秒或 4096 点写一个 IndexedDB 块，不读取 60 秒环形缓冲。
- 数据库 `dicar-tune-recordings` v1：`recordings` 元数据 + `[recordingId, chunkIndex]` 原始批次块。单次 5 分钟，最多 20 条 / 256 MiB；保护回放/导出 ID，自动清理最旧完整记录。
- 手动、到期、暂停、断线、订阅变化均封存；订阅变化先停记录再发新订阅。写失败删除整条本次记录，启动时清理异常退出留下的非 complete 数据。
- schema v1 JSON 可导入导出；导入全量验证后单事务写入，重复 ID 重新生成。CSV 按采样时刻展开并防公式注入。
- 回放使用独立只读 `TelemetryRingBuffer`，支持拖动、单步、0.25×/0.5×/1×/2×/4×和到尾暂停，不暂停设备、不替换实时 store、不发送 Bridge 命令。

### 精准控制台 UI 与旧首页兼容（`ebacc2a`–当前）
- 顶部保留“概览、实时调试、波形记录、诊断”四个真实入口；首页恢复 2×2 的“实时调参与波形、数据记录与回放、参数方案、连接与链路诊断”四卡信息架构，并保留当前车辆和最近记录摘要。
- 设备状态芯片打开 `ConnectionDrawer`，连接、硬件指南和车型偏好集中在唯一抽屉。旧 `ConnectionStatusBar` 和占位页保持删除，不恢复第二套连接状态。
- 参数方案卡进入 `/live?panel=snapshots` 并复用现有 `SnapshotManagerDialog`；旧 `/parameter-sets` 书签使用 replace 重定向到同一入口，关闭面板会清理 `panel` 且保留其他查询参数。
- `FirmwareFlashEntry` 已接入独立固件向导与 Tauri Firmware service；只有已就绪的
  HC-05/nanoUART-wl 9600 baud 真实串口可打开，浏览器保持 unavailable。
- `/records` 是真实独立页面，复用既有 `RecordingController`；首页摘要只显示真实设备/车型/固件/参数/遥测/存储代和最近完整记录，四张卡均指向可执行的真实能力。
- 实时工作台提供标准/赛道模式，DOM 顺序和组件实例不变；模式切换只改布局密度，测试确认不会发送订阅、暂停、写参数或固化命令。实时指标条只读取现有目标/反馈/误差/订阅/丢样/RTT，缺值为“—”；零 dirty 时不渲染底部变更条。
- 诊断按设备健康、连接质量、协议事件组织现有快照，原始计数折叠展示；窄屏导航、焦点环、减弱动效和明确的触控尺寸已覆盖。
- 当前门禁为 44 个 Vitest 文件 / 194 项测试和 11 个既有 Playwright 场景；新增
  固件向导由 Vitest 覆盖，真实串口和 ROM BSL 不进入浏览器 E2E。
- 无线烧录阶段已经修改 Rust/Tauri、DCTP wire、C 参考库与 React；普通参数与
  遥测语义保持不变。新增黄金向量尚待用户确认后落盘。

## 3. 0.2.0 发布状态

- `DiCar-Tune-0.2.0-Windows-x64-Setup.exe`：3,181,621 字节，SHA-256 `30608FF384F05CCA2C2AF58BE0A06FAA22EB19EB22DDA56390DCDC267A24EDEF`。
- `DiCar-Tune-0.2.0-Windows-x64-Portable.exe`：12,360,192 字节，SHA-256 `BBB333A5372A5405D5E9B19D521EF46D503DCE3480B360C4FB899978E11337F7`。
- 最终 `release/` 便携版隐藏启动冒烟：精确 PID 3336 持续响应并拥有 `127.0.0.1:63056`，随后只终止该 PID并确认退出。
- 两个 0.1.2 实体文件在 0.2.0 全部检查成功后移入 Windows 回收站；`release/` 现在只保留两个 0.2.0 产物和校验清单。

参数方案导入明确不属于 0.2.0，也不再列入后续待办。无线烧录软件首版已实现；
nanoUART-wl/HC-05 与天猛星实板验证仍未执行。云协作、macOS/Linux/移动端等仍不在范围内。

## 4. 环境与门禁（Windows 11，本机已验证）

- pnpm 全局可用；Node 26 兼容（`.node-version` 仅作推荐）；`git safe.directory` 已配置；MSVC 工具链可用（cc crate 编译 C 库依赖它）。
- 前端门禁（在 `apps/dicar-desktop/` 或用 `--filter`）：`pnpm lint`、`pnpm typecheck`、`pnpm test -- --run`、`pnpm build`、`pnpm test:e2e`。最近 Vitest 基线为 44 个文件 / 194 个测试，既有 Playwright 为 11 个场景。
- Rust 门禁（仓库根）：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`（含 C 交叉验证）、Tauri `native-check`、`cargo run -p dctp-sim --bin generate_vectors -- --check`（目标为八个黄金向量）。
- 本机 Smart App Control 会误拦个别 Cargo 中间产物哈希；未关闭安全策略。测试目标用等价的包级命令补齐，向量检查使用 release profile，打包仅设置 `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_DEBUG=1` 改变 build script 哈希。
- AI 覆盖使用内存凭据替身和本地 HTTP server；前端/Playwright 使用 Mock，不访问真实 DeepSeek。记录域使用 `fake-indexeddb` 覆盖原子导入、清理、失败回滚和容量边界。
- 全绿是提交前提；rustfmt 对新 Rust 代码常需先 `cargo fmt --all`。

## 5. 不可破坏的设计约束（改代码前必读）

- **协议 wire 格式变更必须过规格评审并重新生成黄金向量**；`test-vectors/dctp-v1/` 是跨端契约，C 库与 Rust 同时受它约束。
- 参数 ACK 是 RAM/Revision 的唯一权威；UI 不得乐观更新；固化必须原子（canonical CRC + 冻结集合）。
- 断线不回滚 RAM、不伪造"已保存"；未固化项显示"设备状态未知"。
- 车型 YAML 只组织展示，永不覆盖 Manifest 的类型/范围/可写性。
- AI 及任何自动化：只写 RAM、本地校验不可绕过、固化永远人工确认。
- React 只消费 bridge/store；协议与安全边界在 Rust 核心，UI 限制只是引导。

## 6. 新会话建议开场

1. 读本文件。
2. `git log --oneline -15` 对照第 2 节确认基线未漂移。
3. 先完成八个黄金向量落盘与全量门禁，再进入实体硬件验证；不重建当前发布产物。
4. 实板按正常升级、擦除/写入中断、BSL+RST 恢复和回滚四项记录证据，任何一项
   未跑都保持“未验证”。
5. MSPM0G3519 与 STM32F1/F4 不在首版实现范围；需要独立目标适配和测试。
6. 如需改协议/C 库，先重读第 5 节和规格对应章节。
