# DCTP v1 车端参考库（C99）

这是 DiCar Tune 车端协议栈的参考实现：把本目录的 `include/` 与 `src/` 两个
`.c` 一个 `.h` 加入你的 MCU 工程，注入串口发送、Flash 写入和时钟回调，车辆
即可被 DiCar Tune 桌面端发现、调参、订阅波形并固化参数。

- 纯 C99，无动态分配、无操作系统依赖、无 `printf`。
- 线上格式与行为以仓库 `crates/dctp-protocol` 与 `crates/dctp-sim` 为权威；
  `crates/dctp-device-c` 在每次 `cargo test --workspace` 时用黄金向量和
  Rust 协议栈对本库做逐字节交叉校验。
- 已实现：会话（HELLO/心跳/关闭/3000 ms 失效）、Manifest 分片、参数读写与
  Revision 冲突、Flash 固化（canonical CRC32 + A/B 双槽 + 幂等 ACK）、
  1–8 通道遥测批次、结构化日志、可靠请求幂等缓存（32 项）。
- 未实现：`PREPARE_FLASH`（返回 `UNKNOWN_MESSAGE`，与内置模拟器一致；
  无线烧录协调属于后续阶段）。

## 1. 最小接入

```c
#include "dctp_device.h"

/* 1) 参数表：param_id 严格升序。字符串限长见头文件注释。 */
static const dctp_param_descriptor_t PARAMS[] = {
  {
    .param_id = 1,
    .type = DCTP_TYPE_F32,
    .flags = DCTP_PARAM_WRITABLE | DCTP_PARAM_PERSISTENT,
    .machine_name = "pid.kp", .display_name = "速度 Kp", .group = "控制", .unit = "",
    .default_value = {DCTP_TYPE_F32, {.f32 = 1.0f}},
    .constraint_kind = DCTP_CONSTRAINT_NUMERIC,
    .min = {DCTP_TYPE_F32, {.f32 = 0.0f}},
    .max = {DCTP_TYPE_F32, {.f32 = 1000.0f}},
    .step = {DCTP_TYPE_F32, {.f32 = 0.01f}},
  },
};

/* 2) 遥测通道：channel_id 严格升序，每个通道一个快速取值函数。 */
static uint32_t read_speed(void *user) { (void)user; return dctp_f32_bits(g_speed_mps); }
static const dctp_channel_descriptor_t CHANNELS[] = {
  {200, DCTP_TELEMETRY_F32, "drive.speed_mps", "车辆速度", "驱动", "m/s", read_speed},
};

/* 3) 平台回调。 */
static void uart_write(void *user, const uint8_t *bytes, size_t len);  /* 入发送环形缓冲 */
static size_t uart_tx_free(void *user);                                /* 环形缓冲剩余空间 */
static int flash_persist(void *user, const uint8_t *blob, uint32_t len); /* 写非活动槽并读回 */

static dctp_device_t g_device; /* 静态分配即可 */

void app_init(void) {
  dctp_device_config_t config = {0};
  config.params = PARAMS;         config.param_count = 1;
  config.channels = CHANNELS;     config.channel_count = 1;
  memcpy(config.device_id, "MY-CAR-0000000001", DCTP_DEVICE_ID_LEN); /* 取前 16 字节 */
  config.boot_count = read_boot_count();
  config.firmware_major = 1;
  config.write = uart_write;
  config.tx_free = uart_tx_free;  /* 可为 NULL：不做遥测/日志预算 */
  config.persist = flash_persist; /* 可为 NULL：设备不支持固化 */

  if (!dctp_device_init(&g_device, &config)) { /* 描述表不合法 */ }
  dctp_storage_apply(&g_device, slot_a_bytes, slot_a_len, slot_b_bytes, slot_b_len);
}

void app_main_loop(void) {
  uint8_t bytes[64];
  size_t n = uart_ring_pop(bytes, sizeof bytes);  /* ISR 只进环形缓冲 */
  dctp_device_rx(&g_device, bytes, n, millis());  /* 主循环里解析并回应 */
  dctp_device_poll(&g_device, millis(), micros());/* 推进遥测与会话失效 */
}
```

（`dctp_f32_bits` 表示用 `memcpy` 取 float 位模式；参考头文件即可，示例从简。）

## 2. 回调契约

| 回调 | 必需 | 契约 |
| --- | --- | --- |
| `write` | 是 | 把整段字节交给 UART 发送路径。可靠响应经此发出，必须被完整接受：要么发送环形缓冲足够大（建议 ≥ 2 KiB），要么在此阻塞到放完。不得在回调里丢弃部分字节。 |
| `tx_free` | 否 | 返回发送路径当前可接受的字节数。遥测批次与日志在发送前检查；空间不足时整帧丢弃并计入缺口，绝不发半帧，保证 P0/P1 响应不被 P2/P3 挤占。传 NULL 表示不限制。 |
| `persist` | 否 | 收到完整槽记录（`DCTP_STORAGE_BLOB_MAX` 以内）。实现应写入**非活动**槽、读回校验，成功返回 `DCTP_PERSIST_OK`。写失败返回 `DCTP_PERSIST_STORAGE_FAILED`，读回不一致返回 `DCTP_PERSIST_VERIFY_FAILED`。库只有在成功后才更新 Flash 影子值并递增 Generation；同一请求重试由幂等缓存保证不会写两次。传 NULL 时设备不上报 PERSISTENCE 能力。 |

时钟：`now_ms` 用于会话 3000 ms 失效（单调毫秒，可回绕）；`now_us` 用于遥测
节拍（单调微秒，可回绕，两次 `poll` 间隔不得超过半个回绕周期——实践中只需
每个主循环调用一次）。

线程/中断边界：库不做任何加锁。`dctp_device_*` 的全部调用必须在同一上下文
（通常是主循环）。串口 ISR 只把收到的字节放进自己的环形缓冲，由主循环取出
后交给 `dctp_device_rx`。

## 3. A/B 双槽持久化

`persist` 收到的槽记录格式（全部小端）：

```text
u32 magic = 0x31565044 ("DPV1")
u16 storage_version = 1
u16 reserved = 0
u32 manifest_crc32
u32 generation           递增代号
u32 payload_len
    payload: 重复 { u32 param_id, u8 type, 值字节 }
u32 crc32                覆盖 magic..payload（CRC-32/ISO-HDLC）
```

建议实现：Flash 里划两个等大扇区轮流写。启动时把两个扇区的原始字节交给
`dctp_storage_apply`，库会选出 CRC 合法且 Generation 较新的槽并恢复参数；
固件升级后参数表变化时，按 `param_id` + 类型匹配的条目仍然生效，其余忽略。

## 4. 资源预算

`dctp_device_t` 全静态，大小由编译期宏决定。默认配置
（`DCTP_MAX_PAYLOAD=1024`、64 参数、16 通道、32 项缓存）约 7 KiB RAM，
代码约 10 KiB Flash（Cortex-M -Os，实测以你的工具链为准）。收紧示例：

```c
/* 编译选项中覆盖，收发缓冲、缓存与参数状态数组随之缩小 */
#define DCTP_MAX_PAYLOAD 256   /* 桌面端会按 HELLO 协商自动适配 */
#define DCTP_MAX_PARAMS 32
#define DCTP_MAX_CHANNELS 8
```

`DCTP_MAX_PAYLOAD=256` 时整机 RAM 占用约 4 KiB。注意规格要求幂等缓存至少
32 项，`DCTP_REQUEST_CACHE_ENTRIES` 不应低于 32。

## 5. 移植检查单

1. UART 波特率与桌面端一致（nanoUART-wl 建议 460800）；8N1；ISR 进环形缓冲。
2. `write` 的发送缓冲足够容纳一个最大帧（`DCTP_MAX_PAYLOAD=1024` 时约 1.1 KiB）。
3. `millis()`/`micros()` 单调；控制环里用 `dctp_device_set_value` 改参数
   （它会递增 Revision，桌面端据此感知固件内部修改）。
4. Flash 槽实现读回校验后才返回成功；掉电测试两个槽轮流写。
5. 主循环频率至少高于遥测采样率的一半（500 Hz 订阅建议 ≥ 1 kHz 循环，
   每次 `poll` 最多发一批 16 个样本）。
6. 参数/通道描述表与车型 YAML 里的 `machine_name` 精确一致（区分大小写）。
7. 用桌面端"模拟器体验"先熟悉流程，再切"真实串口"接入你的车。

### AI 调参额外要求

要让桌面版 AI 向导能够运行阶跃实验，车端除了增益参数和反馈遥测，还必须把
控制目标（例如目标速度）暴露为**可写数值参数**。建议给目标参数设置
`DCTP_PARAM_DANGEROUS`，提醒操作者它会立即改变车辆行为；同时不要设置
`DCTP_PARAM_PERSISTENT`，避免实验激励进入 Flash 待固化集合。

车型 YAML 的 `control_loops[].target_parameter`、`gains` 和
`telemetry.feedback` 都按区分大小写的完整 `machine_name` 绑定。参数表、遥测
通道表和 YAML 中任一名称不一致，向导都会拒绝启动，不会退回显示名猜测绑定。
AI 仍只写 RAM；稳定结果是否固化到 Flash 必须由操作者人工审阅确认。

## 6. 与仓库测试的关系

`crates/dctp-device-c` 把本目录的 C 源编译进 Rust 测试并做两类校验：

- `tests/golden.rs`：C 编码器复刻 `test-vectors/dctp-v1/` 的六个黄金帧，逐字节比对。
- `tests/behavior.rs`：Rust 协议栈构造请求驱动 C 设备，覆盖握手、重放、
  幂等、参数校验链、固化原子性、遥测节拍与丢弃、超时与重同步等 20 个场景。

修改本库后运行：

```powershell
cargo test -p dctp-device-c
```

协议或黄金向量的任何变更都必须先经 `docs/superpowers/specs/` 的规格评审。
