# STM32F103 巡线车 App 测试固件设计

## 目标

在 `C:\stm32\DIcai_TS` 创建一个可由 Keil 直接编译、下载到
STM32F103C8T6 Blue Pill 的最小巡线车工程。固件既能驱动实车低速巡线，也能
通过单个 USB-TTL 与 DiCar Tune App 建立 DCTP v1 会话、读写参数并显示实时
波形。

这是 App 硬件联调目标，不是完整竞赛车固件。工程优先保证接线验证、可观察性
和安全停机，不提前加入后续功能。

## 明确不做

- 不使用 RTOS、动态内存、C++ 或额外第三方库。
- 不实现速度 PI、里程控制、Flash 参数保存、无线烧录、AI 自动调参或电池 ADC。
- 不增加第二串口、复杂日志框架、配置系统或未被当前硬件使用的驱动。
- 不修改 DiCar Tune App 或 DCTP v1 协议。

## 工具链与目录

- STM32CubeMX 6.17，生成 STM32F103C8Tx HAL 工程。
- Keil MDK 5.39，使用已安装的 STM32F1 DFP 2.4.1。
- CubeF1 HAL 1.8.7，只复制工程实际使用的 HAL 文件。
- 目标目录固定为 `C:\stm32\DIcai_TS`。
- DCTP 直接引用 `C:\DiCar_LAB\firmware\dctp-device` 的一个头文件和两个 C
  源文件，不复制整仓库。

## 引脚

| 功能 | STM32 引脚 |
| --- | --- |
| 左编码器 A/B | `PA0 / PA1`，TIM2 Encoder |
| 右编码器 A/B | `PB6 / PB7`，TIM4 Encoder |
| TB6612 PWMA/PWMB | `PA6 / PA7`，TIM3 CH1/CH2，20 kHz |
| TB6612 AIN1/AIN2 | `PB12 / PB13` |
| TB6612 BIN1/BIN2 | `PB14 / PB15` |
| TB6612 STBY | `PA8`，外部 10 kΩ 下拉 |
| 灰度 AD0/AD1/AD2 | `PB0 / PB1 / PB5` |
| 灰度 OUT | `PA4` |
| OLED SCL/SDA | `PB10 / PB11`，I2C2 |
| USB-TTL TX/RX | `PA10 / PA9`，USART1，115200 8N1 |
| 按键 | `PA12`，内部上拉，按下为低 |
| 低电平蜂鸣器 | `PB8`，高电平关闭、低电平鸣响 |
| 板载 LED | `PC13`，低电平点亮 |
| SWD | `PA13 / PA14` |

`PA5` 保持未使用。串口模块按实物使用四根线：VCC 接 3.3V、GND 共地、TX
接 PA10、RX 接 PA9。

## 程序结构

使用 HAL 裸机主循环，不建立任务调度器。SysTick 提供毫秒时间，主循环根据时间
戳调用以下固定周期工作：

- 5 ms：扫描一次八路灰度。每次切换 AD0—AD2 后等待约 60 us，再读取 OUT。
- 10 ms：计算巡线误差并执行 PD 差速控制。
- 20 ms：读取 TIM2/TIM4 增量，更新累计计数和 counts/s。
- 100 ms：刷新 OLED、LED 和蜂鸣器状态。
- 每次主循环：消费 USART1 接收环形缓冲并调用 DCTP poll。

只保留五个手写模块：

1. `car_app`：STOP、RUN、LINE_LOST 三态和周期调度。
2. `line_control`：灰度加权误差、PD 修正和 PWM 限幅；保持纯 C、无 HAL 依赖。
3. `board_io`：电机、编码器、灰度、按键、蜂鸣器和 OLED 的最小硬件访问。
4. `oled`：只实现本工程需要的 SSD1306 文本显示。
5. `dctp_port`：参数表、遥测表、USART1 收发和 DCTP 调用。

## 巡线和安全行为

八个探头按从左到右权重 `-7, -5, -3, -1, 1, 3, 5, 7` 计算加权平均
误差。控制器每 10 ms 计算：

```text
correction = kp * error + kd * (error - previous_error)
left_pwm   = clamp(base_pwm + correction)
right_pwm  = clamp(base_pwm - correction)
```

PWM 以百分比表示，最终限制在 `0%..60%`。首版不倒车修线。

上电顺序固定为 PWM=0、STBY=0、蜂鸣器关闭。物理按键或 App 参数都可切换
`control.enabled`；物理按键改变状态时同步更新 DCTP 参数，使 App 显示一致。
连续 100 ms 没有任何有效灰度位时进入 LINE_LOST：PWM 清零、STBY 拉低并短鸣，
必须再次按键或由 App 重新启用才能运行。

## App 参数

只定义四个 RAM 参数，不提供持久化回调：

| machine_name | 类型 | 默认值 | 范围 |
| --- | --- | --- | --- |
| `control.enabled` | bool | false | 启停，危险参数 |
| `drive.base_pwm` | f32 | 20% | 0%..60% |
| `line.kp` | f32 | 4.0 | 0..20 |
| `line.kd` | f32 | 2.0 | 0..20 |

## App 波形

定义十六个遥测通道。DCTP 每次订阅最多八路，App 可在两组之间切换；推荐
50 Hz 订阅，控制环仍保持 100 Hz。

### 灰度组

- `sensor.line_0` 至 `sensor.line_7`：八个 0/1 方波。

### 控制组

- `sensor.line_bits`
- `sensor.line_error`
- `encoder.left_count`
- `encoder.right_count`
- `encoder.left_cps`
- `encoder.right_cps`
- `motor.left_pwm`
- `motor.right_pwm`

## DCTP 移植约束

- 编译期设置 `DCTP_MAX_PAYLOAD=256`、`DCTP_MAX_PARAMS=4`、
  `DCTP_MAX_CHANNELS=16`；幂等缓存保持协议要求的 32 项。
- `persist=NULL`、`prepare_flash=NULL`，不声明固化和固件烧录能力。
- USART1 RX 中断只把字节写入 256 字节环形缓冲；DCTP 解析全部在主循环。
- 首版发送回调使用完整阻塞发送，以最少代码满足 DCTP 不允许半帧的契约。
  波形推荐限制为 50 Hz；只有实测阻塞影响控制环时才改为 TX 环形缓冲。
- DCTP 参数和遥测 machine_name 必须与本设计完全一致。

## 文件边界

目标目录只包含 CubeMX/Keil 编译所需文件、上述五个手写模块、一个无框架的主机
测试程序和一份接线/运行 README。不生成示例工程副本、压缩包、构建缓存、日志
目录或备用配置。

## 验证

1. 主机测试先验证灰度误差、PD 左右输出、60% 限幅和丢线状态；测试必须先失败
   再实现生产代码。
2. 使用 Keil 命令行构建，要求零编译错误；记录无法消除的厂商警告。
3. 运行现有 `cargo test -p dctp-device-c`，确认引用的协议库仍通过测试。
4. 实板依次验证：安全上电、OLED/按键/蜂鸣器、八路灰度、两个编码器、悬空低
   PWM 点动、App 握手与两组波形、最后低速落地巡线。

