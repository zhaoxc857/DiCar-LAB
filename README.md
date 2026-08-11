# DiCar LAB

DiCar LAB 的首版是面向电子设计大赛和智能车竞赛的无线调参工作台。主页采用菜单式入口，实时工作台提供参数编辑、编码器标定、最多 8 路波形、RAM/Flash 状态和本地演示权限控制。

## 运行 Web 初版

需要 Node.js 22 和 pnpm 11：

```text
pnpm install --frozen-lockfile
pnpm dev
```

浏览器打开 `http://127.0.0.1:5173/`。纯 Web 环境默认使用确定性模拟设备，可直接体验“连接 → 调参 → 波形 → 审阅固化”的完整流程。支持 Web Serial 的浏览器可授权并识别 USB 串口，但浏览器端 DCTP 会话仍在下一切片接入；在握手完成前不会显示真实设备已连接，完整硬件调参暂时使用桌面 App。

Windows 桌面 App 内置三种真实串口配置：nanoUART-wl、HC-05 蓝牙串口和通用串口。只有完成 DCTP HELLO、Manifest 和参数加载后，界面才会显示设备已就绪；仅打开端口或握手失败都会保持未连接状态。无线模块负责透明传输，车端 MCU 仍需运行 DCTP 固件。

### nanoUART-wl

1. 电脑端插入 USB，车端连接 `3V3`、`GND`、`TX`、`RX`，其中 TX/RX 交叉并确保共地。
2. 在 App 中选择“真实串口 → nanoUART-wl → 新增的 COM 口”。推荐选择“自动探测”；顺序为 460800、230400、115200 baud。
3. 460800 baud 下允许最多 8 通道 × 500 Hz；较低速率会自动降低遥测安全上限。

### HC-05（电脑蓝牙直连车端）

1. 在 Windows 蓝牙设置中先与车端 HC-05 配对，并在“更多蓝牙设置 → COM 端口”确认系统创建的**传出（Outgoing）COM**。
2. HC-05 的 TX 接 MCU RX，RX 接 MCU TX，并与 MCU 共地。HC-05 UART 按 3.3 V 逻辑处理；5 V MCU 发往 HC-05 RX 时必须分压或使用电平转换。
3. 在 App 中选择“真实串口 → HC-05 蓝牙串口 → 传出 COM → 自动探测”。探测顺序为 115200、9600、38400、57600、230400、460800 baud，成功后才保存配置。
4. 普通 HC-05 链路默认限制为 4 通道 × 50 Hz；9600 baud 限制为 2 通道 × 10 Hz。HC-05 是经典蓝牙 SPP 虚拟串口，纯 Web 客户端不承诺可用。

如果自动探测全部失败，请依次检查：选中的是否为传出 COM、车端 MCU 与模块波特率是否一致、TX/RX 是否交叉、是否共地，以及 MCU 固件是否已启用 DCTP。

关键前端验证：

```text
pnpm lint
pnpm typecheck
pnpm --filter @dicar/desktop test --run
pnpm build
pnpm test:e2e
```

Windows 安装包命令为 `pnpm --filter @dicar/desktop tauri:build`，需要 Visual Studio C++ Build Tools 和正式应用图标。当前发布版本为 0.1.2。

## DCTP v1 protocol foundation

`dctp-protocol` contains the DCTP v1 wire codec and payload models. It performs no serial I/O.
`dctp-sim` is the deterministic TCP test transport used by the next desktop-client plan.

Parameter reads report the current RAM value and, for persistent parameters, the separately
committed flash value. `PARAM_COMMIT_ACK` reports the canonical CRC and storage generation.
The simulator provides deterministic, time-varying telemetry across its default drive channels.

Developer commands:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p dctp-sim -- --help
cargo run -p dctp-sim -- --listen 127.0.0.1:7100
cargo run -p dctp-sim --bin generate_vectors -- --check
```

`generate_vectors` commits six DCTP v1 golden frames, including `param-value.bin` and
`param-commit-ack.bin`; run it without `--check` only when intentionally regenerating them.
