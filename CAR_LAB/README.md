# CAR LAB（应用主目录）

这是 **DiCAR LAB** 上位机的核心应用目录（程序入口 `main.py`）。

- 项目总说明、功能特性、快速开始、通信协议见仓库根目录的 [`../README.md`](../README.md)。
- 版本历史见 [`../CHANGELOG.md`](../CHANGELOG.md)。
- 各版本详细说明与 MCU 通信手册见 [`docs/`](docs/)。

## 从源码运行

```bash
python -m venv .venv
.venv\Scripts\activate        # Windows；macOS/Linux: source .venv/bin/activate
pip install -r requirements.txt
python main.py
```

启动后在顶部「连接方式」选择 **仿真** 即可无硬件体验全部功能。

## Windows 桌面发布

普通用户无需从本目录运行源码。请从 [GitHub Releases](https://github.com/zhaoxc857/DiCar-LAB/releases) 下载 `DiCAR-LAB-v1.13.0-Windows-x64-onefile.exe`（单文件版，双击即用）或 `DiCAR-LAB-v1.13.0-Windows-x64.zip`（便携 ZIP 版，完整解压后运行 `DiCAR LAB.exe`）。发布包通过根目录的 `build_portable_windows.bat` / `build_onefile_windows.ps1` 生成。

固件烧录页支持 STM32F1/F4（stm32flash）与 TI MSPM0G3507（内置 TI ROM BSL 驱动）的无线烧录：每次烧录自动快照进固件版本库（可加备注、一键回退），写入进度以进度条显示，擦除/写入/校验各阶段可取消。STM32 与 MSPM0 均需按烧录页指引把车辆置于 bootloader 模式。

## 安全提示

上位机不是唯一安全层。真实车辆 MCU 必须独立实现通信超时停车、输出限幅、异常保护和必要的物理急停。
