# DiCar Tune 公开文档设计

## 目标

为 DiCar Tune 0.1.2 建立一套面向参赛使用者和开发者的公开文档入口。README 保持简洁，详细操作与开发资料拆分，所有描述必须与当前已实现、已验证的功能一致。

## 文件结构

### `README.md`

README 是项目门户，包含：

- 项目定位、当前版本和主要能力；
- Windows 安装版与便携版的本地下载入口；
- 使用内置模拟器完成首次体验的最短流程；
- nanoUART-wl、HC-05 和通用串口的兼容性摘要；
- 用户手册和开发文档入口；
- 当前限制，明确纯 Web DCTP、云协作、AI 调参和多车并发尚未完成。

### `docs/user-guide.md`

用户手册覆盖：

- 安装版与便携版启动方式；
- 内置模拟器连接；
- 车端 MCU 必须运行 DCTP 固件的前提；
- nanoUART-wl 接线、COM 选择和自动波特率探测；
- HC-05 Windows 配对、传出 COM、3.3 V 电平安全与自动探测；
- RAM 写入、Flash 固化、编码器参数和波形操作；
- HC-05 与低波特率遥测上限；
- 连接拒绝、无 COM、握手失败和波形受限等常见问题。

### `docs/development.md`

开发文档覆盖：

- React/Tauri、App Core、DCTP Protocol 和 Simulator 的职责；
- Node.js、pnpm、Rust stable 和 Windows C++ Build Tools 要求；
- Web、桌面和模拟器的开发命令；
- Rust、前端、Playwright 和协议黄金向量验证命令；
- Windows NSIS 打包与发布文件位置；
- 修改协议或硬件配置时必须保持的安全边界。

## 内容原则

- 中文为主，协议名、字段名和命令保留英文。
- 已实现能力使用肯定语气；计划能力必须标为“尚未实现”或“后续版本”。
- 无线串口模块只负责透明传输，不暗示其能替代车端 DCTP 固件。
- 不承诺未经实体硬件验证的连接质量、距离或最高吞吐量。
- 下载链接使用仓库同级 `release` 目录中现有 0.1.2 文件。

## 验收

- README 中的所有相对文档链接可解析。
- 安装版、便携版和两份指南均可从 README 一步到达。
- 文档中的命令与 `package.json`、Cargo workspace 和 Tauri 配置一致。
- 文档中不存在占位符、失效版本号或将计划功能写成已完成功能的表述。
