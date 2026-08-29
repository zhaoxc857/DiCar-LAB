# DiCAR_LAB 迁移与 STM32 协议设计

## 目标

以 IKUN-CAR-LAB 为新的桌面端基础，原地替换旧 DiCAR_LAB，并让现有 STM32F103C8T6 巡线车通过 HC-05 串口稳定连接、调参和查看波形。

## 边界

- 保持项目简单，只服务于 APP 硬件联调。
- 保留旧项目的 Git 完整历史和少量有价值的未跟踪资料作为独立备份。
- 不迁移旧 APP 的构建缓存、发布物和依赖目录。
- STM32 继续使用现有 `car_app.c` 对外接口，不重构已验证的电机、编码器、灰度和 OLED 逻辑。
- 暂无电池 ADC。

## 桌面端

- 根目录和用户可见名称统一为 DiCAR_LAB / DiCAR LAB。
- 保留 IKUN-CAR-LAB 已有的 Python/PySide6 架构和逐行 JSON 协议实现。
- 新增专用车辆配置，只暴露 `control_enabled`、`base_pwm`、`line_kp`、`line_kd` 四个必要参数。
- 波形覆盖巡线误差、左右轮速度、左右 PWM、灰度位图；8 路原始灰度和累计编码器低频发送。

## 固件协议

串口为 USART1，HC-05 默认 9600 baud，帧格式为 UTF-8/ASCII JSON 加换行：

- `GET`: 读取参数。
- `SET`: 修改 RAM 参数。
- `CMD`: 控制类命令。
- `ACK`: 固件对 GET/SET/CMD 的确认。
- `TEL`: 固件主动遥测。

固件只解析 APP 实际会发送的固定字段，不引入通用 JSON 库。发送使用 UART 中断和固定缓冲区，避免阻塞控制循环；ACK 优先于遥测。9600 baud 下核心波形约 4 Hz，详细灰度和累计计数约 1 Hz。

## 安全与兼容

- 未识别参数返回 `ok:false`，不修改运行状态。
- 参数范围仍由固件钳位。
- 蓝牙链路断开不会自动启动电机；`control_enabled` 保持显式控制。
- 保留现有 `dctp_port_*` C API 名称以减少对已验证业务代码的改动，但其内部协议改为 JSON Line。
