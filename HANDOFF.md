# DiCar Tune 开发交接（2026-08-14）

给新会话/新开发者的现状快照。读完本文 + 按需查阅引用文件，即可继续开发。

- 基线：`main` @ `cb095f0`，工作区干净，全部质量门禁绿。
- 当前发布版本 0.1.2（release/ 目录）；其后新增的三个大功能**尚未打包发布**。
- 权威规格：[docs/superpowers/specs/2026-08-10-dicar-serial-collaboration-protocol-design.md](docs/superpowers/specs/2026-08-10-dicar-serial-collaboration-protocol-design.md)（DCTP v1 协议与分阶段计划）。
- 开发文档：[docs/development.md](docs/development.md)（架构、环境、门禁、打包）；用户手册：[docs/user-guide.md](docs/user-guide.md)。

## 1. 路线图裁剪（用户 2026-08-13 决定，勿再推荐被裁项）

- **无硬件**：纯软件推进；实板移植、无线模块资格测试、PREPARE_FLASH 全部暂缓。
- **阶段 4（云协作/团队权限/租约/审计）不做；阶段 5（无线烧录）不做。**
- **阶段 6 只做 AI 调参**，用外接 DeepSeek API（用户自备 Key）；不做 macOS/Linux/移动端/Web Serial 真实会话。

## 2. 本阶段完成的功能（规格阶段 1 全部 + 阶段 2 协议栈 + 阶段 3 大半 + AI 调参）

### 车端 C 参考库（`8177a93`）
- [firmware/dctp-device](firmware/dctp-device/README.md)：纯 C99、零动态分配的 DCTP v1 设备栈（会话/Manifest 流式分片/参数校验链/canonical CRC 固化/A/B 双槽/遥测批次/日志/32 项幂等缓存）。PREPARE_FLASH 未实现（与模拟器一致，返回 UNKNOWN_MESSAGE）。
- [crates/dctp-device-c](crates/dctp-device-c)：`cargo test` 时用 cc 编译 C 源并交叉验证——6 个黄金向量逐字节比对 + 20 个行为场景（Rust 协议栈驱动 C 设备）。**C 库任何改动都被这层锁住。**

### 参数方案与固化记录（`57dfd4d`，规格 §14 首版）
- [src/tuning/snapshots.ts](apps/dicar-desktop/src/tuning/snapshots.ts) 纯逻辑（捕获/按稳定 ID diff/导出 JSON/持久化解析）；[tuningSnapshotStore](apps/dicar-desktop/src/stores/tuningSnapshotStore.ts)（localStorage，64 个/1 MiB 限额）；[SnapshotManagerDialog](apps/dicar-desktop/src/components/workbench/SnapshotManagerDialog.tsx) UI。
- 应用规则（§12.3）：缺失/类型变化/越界/只读/状态未知只列出、不自动写；固化成功自动生成带 storage generation 的 commit 记录（钩子在 [CommitReviewDialog](apps/dicar-desktop/src/components/workbench/CommitReviewDialog.tsx)）。

### AI 自动调参（`cb095f0`，自动 N 轮到收敛）
- 循环：阶跃实验 → 本地指标（[metrics.ts](apps/dicar-desktop/src/tuning/metrics.ts)：上升/超调/整定/稳态误差/振荡）→ DeepSeek 决策（[aiClient.ts](apps/dicar-desktop/src/ai/aiClient.ts)，OpenAI 兼容，temperature 0，强制 JSON）→ 限幅写 RAM。引擎在 [autoTune.ts](apps/dicar-desktop/src/tuning/autoTune.ts)（依赖注入，13 个单测覆盖收敛/截断/看门狗/中止/坏输出）。
- **本地不可绕过的护栏**：增益白名单（dangerous 排除）、Manifest 范围 clamp、单轮步长 ≤ 量程 20%、看门狗（超调 >80% 或振荡 >6 → 回滚本地评分最佳轮并终止）、只写 RAM、结束必写回最佳轮。收敛与最佳轮均以本地判定为准，不信任 AI 自评。
- 入口：工作台 header"AI 调参"（[AutoTuneWizard.tsx](apps/dicar-desktop/src/components/workbench/AutoTuneWizard.tsx)）。要求车型 YAML 控制环声明 `target_parameter`（可写数值参数）+ feedback 遥测通道。DeepSeek Key 存 settingsStore（仅本机）。

## 3. 未完成清单（按建议优先级）

1. **模拟器闭环增强**：`dctp-sim`（Rust，桌面内置）与 `MockBridge`（前端演示）的速度遥测都是合成波形，不响应 PID/目标参数——AI 调参在模拟器上跑得通流程但看不到真实收敛。给模拟器加简单车辆模型（一阶惯性 + PID 由参数驱动），无硬件即可端到端演示 AI 闭环。注意 dctp-sim 改遥测值生成会**影响黄金向量吗？不会**（向量是固定输入构造的帧，与 tick 生成无关），但 `telemetry_value()` 行为变化会影响依赖它的测试。
2. **Tauri HTTP 转发命令**：桌面 WebView 直连 api.deepseek.com 可能被 CORS 拦（未实测）。在 src-tauri 加转发 command（reqwest），aiClient 检测 Tauri 环境时走 invoke。客户端已抽象为 `AiChatClient` 接口，改动小。
3. **参数方案导入**：现只有导出；导入 JSON（复用 `parseStoredSnapshot` 校验）+ 限额/覆盖确认，仿车型 YAML 导入的交互。
4. **波形记录与回放**（阶段 3 剩余大项）：遥测录制、时间轴回放、导出。
5. **打包 0.2.0**：三个新功能进发布版。流程见 development.md §9（版本号四处同步 + NSIS 打包 + release/ 更新 + SHA-256）。
6. 小技术债：DeepSeek Key 明文 localStorage（桌面版可换系统凭据库）；AutoTuneWizard 无 e2e（引擎单测已覆盖）；C 库移植指南可补"目标量应暴露为可写参数以支持 AI 调参"。

## 4. 环境与门禁（Windows 11，本机已验证）

- pnpm 全局可用；Node 26 兼容（`.node-version` 仅作推荐）；`git safe.directory` 已配置；MSVC 工具链可用（cc crate 编译 C 库依赖它）。
- 前端门禁（在 `apps/dicar-desktop/` 或用 `--filter`）：`pnpm lint`、`pnpm typecheck`、`pnpm test`（当前 115 个）、`pnpm build`、`pnpm test:e2e`（当前 7 个）。
- Rust 门禁（仓库根）：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets`（含 C 交叉验证）、`cargo run -p dctp-sim --bin generate_vectors -- --check`。
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
3. 按第 3 节优先级选任务；动协议/C 库前重读第 5 节和规格对应章节。
