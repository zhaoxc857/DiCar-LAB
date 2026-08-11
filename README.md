# DiCar LAB

DiCar LAB 的首版是面向电子设计大赛和智能车竞赛的无线调参工作台。主页采用菜单式入口，实时工作台提供参数编辑、编码器标定、最多 8 路波形、RAM/Flash 状态和本地演示权限控制。

## 运行 Web 初版

需要 Node.js 22 和 pnpm 11：

```text
pnpm install --frozen-lockfile
pnpm dev
```

浏览器打开 `http://127.0.0.1:5173/`。纯 Web 环境默认使用确定性模拟设备，可直接体验“连接 → 调参 → 波形 → 审阅固化”的完整流程。支持 Web Serial 的浏览器可授权并识别 USB 串口，但浏览器端 DCTP 会话仍在下一切片接入；在握手完成前不会显示真实设备已连接，完整硬件调参暂时使用桌面 App。

Windows 桌面 App 中选择“真实串口”，再选择无线调试器对应的 COM 口和波特率（115200、460800 或 921600，默认 921600）。只有完成 DCTP HELLO、Manifest 和参数加载后，界面才会显示设备已就绪；端口打开或握手失败都会保持未连接状态。

关键前端验证：

```text
pnpm lint
pnpm typecheck
pnpm --filter @dicar/desktop test --run
pnpm build
pnpm test:e2e
```

Windows 安装包命令为 `pnpm --filter @dicar/desktop tauri:build`，需要 Visual Studio C++ Build Tools 和正式应用图标。

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
