# DiCAR LAB

> A unified desktop tuning workbench for smart cars (differential / Ackermann / four-wheel …).
> Real-time telemetry oscilloscope, online PID tuning, reliable parameter sync, track & corner analysis — with a built-in simulator so you can try everything without hardware.

**CAR LAB** 是一个通用智能车调车上位机（PySide6 桌面程序）。它把「实时示波器 + 在线 PID 调参 + 可靠参数同步 + 赛道/弯道分析」整合在一个界面里，通过统一的 JSON 串行协议与 MCU 通信，支持串口 / 蓝牙串口 / BLE / TCP，并**内置仿真车**——不接硬件也能完整体验全部功能。

![license](https://img.shields.io/badge/license-MIT-blue) ![python](https://img.shields.io/badge/python-3.10%2B-green) ![gui](https://img.shields.io/badge/GUI-PySide6-orange)

---

## 下载桌面版

Windows 10/11 x64 用户可直接从 [GitHub Releases](https://github.com/zhaoxc857/DiCar_Tune/releases) 下载 `DiCAR-LAB-v1.8.0-Windows-x64.zip`。桌面版已经包含 Python 与运行依赖，无需安装 Python、PySide6 或开发工具。

1. 完整解压 ZIP，不要直接在压缩包内运行。
2. 双击 `DiCAR LAB.exe`。
3. 第一次使用先在顶部把「连接方式」选为「仿真」，再点「连接」。确认界面与示波器正常后，再连接真实车辆。

可同时下载 `SHA256SUMS.txt`，在 PowerShell 中校验文件：

```powershell
Get-FileHash .\DiCAR-LAB-v1.8.0-Windows-x64.zip -Algorithm SHA256
```

输出应与 `SHA256SUMS.txt` 中对应记录一致。

## ✨ 功能特性

- **实时调试**
  - 总览：车速、RPM、航向角、角速度、电池、转向输出关键量一屏掌握
  - 中文示波器：中文曲线名 + 协议 key 辅助；工作组一键选通道（速度 / 航向 / 角速度 / 电源 / 电机 / 循迹）；A/B 双游标测 Δt、鼠标探针、冻结、时间窗、局部/全范围 Y 轴
  - Speed Lab / Heading Lab：速度环、航向外环 + 角速度内环在线整定，误差曲线单独放大
  - 自定义环：任意 PID 环字段映射，支持普通 / 专家两种模式
- **车辆实验**：AI 规则调参与阶跃测试、电源 ADC 监控、单电机实验、多电机一致性 / IMU 零偏检查
- **参数与赛道**：统一读写 MCU 参数、参数方案保存/恢复、赛道工程（圈速 + Corner Analyzer 入弯/出弯/弯道耗时分析）、实验档案与曲线比较
- **可靠参数同步**：发送缓冲区、同 key 合并、`seq` 序号、ACK `ok/error`、超时重试、回读校验、断线保留待发送
- **多种连接**：仿真 / 串口 / 蓝牙串口 / BLE GATT / TCP
- **工具**：系统诊断、协议监视器（TX/RX 原始报文）、MSP（MSPM0 / MSP430）接入帮助

## 🚗 支持的车型

车型由 `CAR_LAB/vehicles/` 下的 YAML 描述，切换车型自动重新加载字段映射与连接默认值：

通用两轮差速、通用四电机差速、Ackermann 舵机转向、**麦克纳姆轮全向车**、**两轮平衡车**、**舵机循迹车**、MSPM0 / MSP430 / STM32 双电机、ESP32 Wi‑Fi、BLE 无线车、双/四电机仿真车，以及**自定义车型模板**。

> **麦轮专属支持**：电机层 FL/FR/RL/RR（`fl_rpm`/`fr_rpm`/`rl_rpm`/`rr_rpm`）、底盘运动解算层 Vx/Vy/Wz（目标 + 实际，方便直接调底盘 PID）、示波器「麦轮运动」专属工作组。

**车型插件化**：既支持旧的扁平文件 `vehicles/<name>.yaml`，也支持插件目录 `vehicles/<name>/config.yaml`——**新增一个车型只需加一个文件夹**，不会因为加功能而丢车型。字段含义与贡献步骤见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## 📦 环境要求

- 桌面版：Windows 10/11 x64，无需安装 Python
- 源码开发：Python **3.10+**（发布构建使用 3.12）
- 源码依赖：PySide6、pyqtgraph、PyYAML、pyserial、bleak（见 `CAR_LAB/requirements.txt`）

## 🚀 快速开始

### 方式 A：桌面便携版（Windows，推荐给使用者）

从 GitHub Releases 下载并解压便携包，双击 **`DiCAR LAB.exe`**。程序不写入系统目录，不需要安装器；整个目录可直接移动或删除。

### 方式 B：手动运行（推荐给开发者）

```bash
cd CAR_LAB
python -m venv .venv
# Windows
.venv\Scripts\activate
# macOS / Linux
# source .venv/bin/activate

pip install -r requirements.txt
python main.py
```

### 第一次先跑仿真

启动后在顶部「连接方式」选择 **仿真**，点 **连接**。此时会有一台虚拟车持续回传遥测数据，你可以立即体验示波器、Speed/Heading Lab、参数同步等全部功能，无需任何硬件。

## 🔌 通信协议（概览）

上位机与 MCU 之间使用 **一行一条 JSON**（UTF-8，`\n` 分隔）：

| 方向 | 类型 | 示例 |
| --- | --- | --- |
| MCU → PC | `TEL` 遥测 | `{"type":"TEL","data":{"actual_rpm":812.3,"yaw":1.2}}` |
| PC → MCU | `SET` 写参数 | `{"type":"SET","key":"speed_kp","value":0.9,"seq":12}` |
| PC → MCU | `GET` 读参数 | `{"type":"GET","key":"speed_kp","seq":13}` |
| PC → MCU | `CMD` 指令 | `{"type":"CMD","key":"target_rpm","value":500}` |
| MCU → PC | `ACK` 确认 | `{"type":"ACK","key":"speed_kp","value":0.9,"seq":12,"ok":true}` |

完整协议与可靠参数同步实现要点见 [`CAR_LAB/docs/MCU_to_PC_通信协议与可靠参数同步开发手册_v1.3.1.md`](CAR_LAB/docs/MCU_to_PC_通信协议与可靠参数同步开发手册_v1.3.1.md)。

### STM32F103 + HC-05 快速连接

1. MCU 与 HC-05 使用 9600 baud 串口，按上方 JSON Line 协议每行发送一条 UTF-8 JSON。
2. Windows 先配对 HC-05，记下系统分配的串口号。
3. 软件选择 `STM32F103 循迹车` 车型与「串口」连接方式，选择对应 COM 口后连接。
4. 先架空车轮并保持急停可用，再验证遥测、ACK 与参数回读；不要在未限幅、未配置通信超时停车时落地运行。

## 🗂️ 目录结构

```
DiCar_Tune/
├── DiCAR_Launcher.py / .bat   # 图形化启动器（建虚拟环境 + 装依赖 + 启动）
├── dicar_lab.spec             # Windows 桌面版冻结配置
├── build_portable_windows.bat # 生成便携 ZIP
├── CAR_LAB/
│   ├── main.py               # 程序入口
│   ├── core/                 # 数据总线、协议、传输(串口/BLE/TCP/仿真)、配置、分析
│   ├── ui/                   # 各功能页面（示波器、Speed/Heading Lab、赛道工程…）
│   ├── vehicles/             # 车型（扁平 *.yaml 或插件目录 <name>/config.yaml）
│   ├── docs/                 # 各版本说明与 MCU 通信协议手册
│   └── requirements.txt
├── LICENSE
├── CHANGELOG.md
└── README.md
```

## ⚠️ 安全提示

上位机**不是唯一安全层**。真实车辆的 MCU 必须独立实现：通信超时停车、输出限幅、异常保护和必要的物理急停。切勿仅依赖上位机保证安全。

### 无限烧录路线图

v1.8.0 已接入真实无线烧录：固件烧录页通过内置 stm32flash 与 HC-05 蓝牙串口直接烧录 STM32 固件（自动断开车辆连接、实时日志、失败诊断）。使用前将车辆 BOOT0 跳线帽置于 1 并断电重启，烧录完成后拨回 0。

后续无限烧录将通过可替换后端扩展，并保留每次烧录前校验、写后验证、可取消、可审计日志和失败即停等约束。启用真实烧录前仍需针对具体芯片、探针、供电与恢复流程单独验证。

## 开发与构建

```powershell
# 运行测试
.\CAR_LAB\.venv\Scripts\python.exe -m unittest discover -s tests -v

# 生成 release\DiCAR-LAB-v1.8.0-Windows-x64.zip
.\build_portable_windows.bat
```

GitHub Actions 会在 pull request 与 main 分支更新时执行测试和 Windows 构建；推送 `v*` 标签时会把 ZIP 与 `SHA256SUMS.txt` 发布到 GitHub Releases。

## 🤝 贡献

欢迎 Issue 与 Pull Request。新增车型（v1.6.1 插件化后）只需在 `vehicles/` 下加一个目录即可；新增控制环可用「自定义环 + 专家模式」直接映射 MCU 字段。

## 📄 许可证

本项目采用 [MIT License](LICENSE) 开源。你可以自由使用、修改、分发与商用，仅需保留版权与许可声明。
