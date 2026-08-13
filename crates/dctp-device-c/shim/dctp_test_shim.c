/*
 * dctp-device-c 测试 shim：为 Rust 交叉验证提供固定设备实例与黄金帧构造。
 * 参数与遥测表逐字段复刻 dctp-sim 的 fixed_manifest，使 C 侧 Manifest 编码
 * 必须与 Rust 权威实现逐字节一致。仅用于测试，不属于固件交付物。
 */
#include <string.h>

#include "dctp_internal.h"

/* ------------------------------------------------------------------ */
/* 与 dctp-sim fixed_manifest 一致的静态描述表 */

#define WRITABLE (DCTP_PARAM_WRITABLE | DCTP_PARAM_PERSISTENT)

static const dctp_enum_option_t QUADRATURE_OPTIONS[] = {
    {1, "1x"},
    {2, "2x"},
    {4, "4x"},
};

#define VAL_F32(value) {DCTP_TYPE_F32, {.f32 = (value)}}
#define VAL_U32(value) {DCTP_TYPE_U32, {.u32 = (value)}}
#define VAL_BOOL(value) {DCTP_TYPE_BOOL, {.boolean = (value)}}
#define VAL_ENUM(value) {DCTP_TYPE_ENUM, {.enum_value = (value)}}
#define NO_VALUE {0, {.u32 = 0}}

static const dctp_param_descriptor_t PARAMS[] = {
    {1, DCTP_TYPE_F32, WRITABLE, "pid.kp", "速度 Kp", "控制", "", VAL_F32(1.0f), DCTP_CONSTRAINT_NUMERIC,
     VAL_F32(0.0f), VAL_F32(1000.0f), VAL_F32(0.01f), NULL, 0},
    {100, DCTP_TYPE_U32, WRITABLE, "encoder.left.ppr", "左编码器 PPR", "编码器", "pulse/rev", VAL_U32(512),
     DCTP_CONSTRAINT_NUMERIC, VAL_U32(1), VAL_U32(1000000), VAL_U32(1), NULL, 0},
    {101, DCTP_TYPE_U32, WRITABLE, "encoder.right.ppr", "右编码器 PPR", "编码器", "pulse/rev", VAL_U32(512),
     DCTP_CONSTRAINT_NUMERIC, VAL_U32(1), VAL_U32(1000000), VAL_U32(1), NULL, 0},
    {102, DCTP_TYPE_ENUM, WRITABLE, "encoder.quadrature_multiplier", "正交倍频", "编码器", "x", VAL_ENUM(4),
     DCTP_CONSTRAINT_ENUM, NO_VALUE, NO_VALUE, NO_VALUE, QUADRATURE_OPTIONS, 3},
    {103, DCTP_TYPE_U32, 0, "encoder.left.cpr", "左编码器 CPR", "编码器", "count/rev", VAL_U32(2048),
     DCTP_CONSTRAINT_NUMERIC, VAL_U32(1), VAL_U32(4000000), VAL_U32(1), NULL, 0},
    {104, DCTP_TYPE_U32, 0, "encoder.right.cpr", "右编码器 CPR", "编码器", "count/rev", VAL_U32(2048),
     DCTP_CONSTRAINT_NUMERIC, VAL_U32(1), VAL_U32(4000000), VAL_U32(1), NULL, 0},
    {105, DCTP_TYPE_BOOL, WRITABLE, "encoder.left.inverted", "左编码器反向", "编码器", "", VAL_BOOL(0),
     DCTP_CONSTRAINT_NONE, NO_VALUE, NO_VALUE, NO_VALUE, NULL, 0},
    {106, DCTP_TYPE_BOOL, WRITABLE, "encoder.right.inverted", "右编码器反向", "编码器", "", VAL_BOOL(0),
     DCTP_CONSTRAINT_NONE, NO_VALUE, NO_VALUE, NO_VALUE, NULL, 0},
    {107, DCTP_TYPE_F32, WRITABLE, "drive.wheel_diameter_mm", "Wheel diameter", "Drive", "mm", VAL_F32(65.0f),
     DCTP_CONSTRAINT_NUMERIC, VAL_F32(1.0f), VAL_F32(1000.0f), VAL_F32(0.1f), NULL, 0},
    {108, DCTP_TYPE_F32, WRITABLE, "drive.gear_ratio", "Gear ratio", "Drive", "ratio", VAL_F32(1.0f),
     DCTP_CONSTRAINT_NUMERIC, VAL_F32(0.01f), VAL_F32(100.0f), VAL_F32(0.01f), NULL, 0},
    {109, DCTP_TYPE_U32, WRITABLE, "encoder.sample_period_us", "编码器采样周期", "编码器", "us", VAL_U32(10000),
     DCTP_CONSTRAINT_NUMERIC, VAL_U32(100), VAL_U32(1000000), VAL_U32(100), NULL, 0},
    {110, DCTP_TYPE_F32, WRITABLE, "encoder.speed_lpf_hz", "编码器速度低通截止频率", "编码器", "Hz", VAL_F32(50.0f),
     DCTP_CONSTRAINT_NUMERIC, VAL_F32(0.0f), VAL_F32(1000.0f), VAL_F32(0.1f), NULL, 0},
    {111, DCTP_TYPE_U32, WRITABLE, "encoder.jump_threshold_counts", "编码器跳变阈值", "编码器", "count",
     VAL_U32(10000), DCTP_CONSTRAINT_NUMERIC, VAL_U32(1), VAL_U32(1000000), VAL_U32(1), NULL, 0},
    {112, DCTP_TYPE_F32, WRITABLE, "encoder.max_credible_rpm", "编码器最大可信转速", "编码器", "rpm",
     VAL_F32(10000.0f), DCTP_CONSTRAINT_NUMERIC, VAL_F32(1.0f), VAL_F32(100000.0f), VAL_F32(1.0f), NULL, 0},
    {113, DCTP_TYPE_BOOL, WRITABLE, "encoder.missing_pulse_detection", "编码器丢脉冲检测", "编码器", "",
     VAL_BOOL(0), DCTP_CONSTRAINT_NONE, NO_VALUE, NO_VALUE, NO_VALUE, NULL, 0},
};

static uint32_t read_channel_value(void *user);

#define CHANNEL(id, type, machine, display, group, unit) \
  {id, type, machine, display, group, unit, read_channel_value}

static const dctp_channel_descriptor_t CHANNELS[] = {
    CHANNEL(200, DCTP_TELEMETRY_F32, "drive.speed_mps", "车辆速度", "驱动", "m/s"),
    CHANNEL(201, DCTP_TELEMETRY_I32, "encoder.left_delta", "左编码器增量", "编码器", "count"),
    CHANNEL(202, DCTP_TELEMETRY_U32, "encoder.left_total", "左编码器总数", "编码器", "count"),
    CHANNEL(203, DCTP_TELEMETRY_FLAGS32, "drive.fault_flags", "驱动故障标志", "驱动", ""),
    CHANNEL(204, DCTP_TELEMETRY_U32, "encoder.right_total", "右编码器总数", "编码器", "count"),
    CHANNEL(205, DCTP_TELEMETRY_F32, "drive.left_wheel_speed_mps", "左轮速度", "驱动", "m/s"),
    CHANNEL(206, DCTP_TELEMETRY_F32, "drive.right_wheel_speed_mps", "右轮速度", "驱动", "m/s"),
    CHANNEL(207, DCTP_TELEMETRY_F32, "drive.target_speed_mps", "目标速度", "驱动", "m/s"),
    CHANNEL(208, DCTP_TELEMETRY_F32, "drive.speed_error_mps", "速度误差", "控制", "m/s"),
    CHANNEL(209, DCTP_TELEMETRY_U32, "motor.left_pwm", "左 PWM", "电机", "permille"),
    CHANNEL(210, DCTP_TELEMETRY_U32, "motor.right_pwm", "右 PWM", "电机", "permille"),
    CHANNEL(211, DCTP_TELEMETRY_I32, "encoder.right_delta", "右编码器增量", "编码器", "count"),
    CHANNEL(212, DCTP_TELEMETRY_U32, "control.loop_jitter_us", "控制环抖动", "控制", "us"),
    CHANNEL(213, DCTP_TELEMETRY_F32, "power.battery_voltage", "电池电压", "电源", "V"),
    CHANNEL(214, DCTP_TELEMETRY_F32, "steering.error_deg", "转向误差", "转向", "deg"),
    CHANNEL(215, DCTP_TELEMETRY_U32, "system.uptime_ms", "运行时间", "系统", "ms"),
};

/* ------------------------------------------------------------------ */
/* shim 实例：设备 + 发送捕获 + 可注入的持久化结果 */

typedef struct {
  dctp_device_t device;
  uint8_t tx[16384];
  size_t tx_len;
  size_t tx_free_bytes;
  bool tx_budget_enabled;
  int persist_result;
  uint32_t persist_calls;
  uint8_t persist_blob[DCTP_STORAGE_BLOB_MAX];
  uint32_t persist_blob_len;
  uint32_t channel_reads;
} dctp_shim_t;

static uint32_t read_channel_value(void *user) {
  dctp_shim_t *shim = (dctp_shim_t *)user;
  shim->channel_reads += 1;
  return shim->channel_reads;
}

static void shim_write(void *user, const uint8_t *bytes, size_t len) {
  dctp_shim_t *shim = (dctp_shim_t *)user;
  if (sizeof shim->tx - shim->tx_len >= len) {
    memcpy(shim->tx + shim->tx_len, bytes, len);
    shim->tx_len += len;
  }
}

static size_t shim_tx_free(void *user) {
  dctp_shim_t *shim = (dctp_shim_t *)user;
  return shim->tx_free_bytes;
}

static int shim_persist(void *user, const uint8_t *blob, uint32_t len) {
  dctp_shim_t *shim = (dctp_shim_t *)user;
  shim->persist_calls += 1;
  if (len <= sizeof shim->persist_blob) {
    memcpy(shim->persist_blob, blob, len);
    shim->persist_blob_len = len;
  }
  return shim->persist_result;
}

size_t dctp_shim_size(void) {
  return sizeof(dctp_shim_t);
}

dctp_shim_t *dctp_shim_init(void *memory, int with_persist, int with_tx_budget) {
  dctp_shim_t *shim = (dctp_shim_t *)memory;
  memset(shim, 0, sizeof *shim);
  shim->tx_free_bytes = 0;
  shim->tx_budget_enabled = with_tx_budget != 0;
  shim->persist_result = DCTP_PERSIST_OK;

  dctp_device_config_t config;
  memset(&config, 0, sizeof config);
  config.params = PARAMS;
  config.param_count = (uint16_t)(sizeof PARAMS / sizeof PARAMS[0]);
  config.channels = CHANNELS;
  config.channel_count = (uint16_t)(sizeof CHANNELS / sizeof CHANNELS[0]);
  memcpy(config.device_id, "DCTP-SIM-DEVICE!", DCTP_DEVICE_ID_LEN);
  config.boot_count = 1;
  config.firmware_major = 1;
  config.firmware_minor = 0;
  config.firmware_patch = 0;
  config.write = shim_write;
  config.tx_free = with_tx_budget != 0 ? shim_tx_free : NULL;
  config.persist = with_persist != 0 ? shim_persist : NULL;
  config.user = shim;

  if (!dctp_device_init(&shim->device, &config)) {
    return NULL;
  }
  return shim;
}

void dctp_shim_rx(dctp_shim_t *shim, const uint8_t *bytes, size_t len, uint32_t now_ms) {
  dctp_device_rx(&shim->device, bytes, len, now_ms);
}

void dctp_shim_poll(dctp_shim_t *shim, uint32_t now_ms, uint32_t now_us) {
  dctp_device_poll(&shim->device, now_ms, now_us);
}

size_t dctp_shim_take_tx(dctp_shim_t *shim, uint8_t *out, size_t capacity) {
  size_t len = shim->tx_len < capacity ? shim->tx_len : capacity;
  memcpy(out, shim->tx, len);
  shim->tx_len = 0;
  return len;
}

void dctp_shim_set_tx_free(dctp_shim_t *shim, size_t bytes) {
  shim->tx_free_bytes = bytes;
}

void dctp_shim_set_persist_result(dctp_shim_t *shim, int result) {
  shim->persist_result = result;
}

uint32_t dctp_shim_persist_calls(const dctp_shim_t *shim) {
  return shim->persist_calls;
}

size_t dctp_shim_last_blob(const dctp_shim_t *shim, uint8_t *out, size_t capacity) {
  size_t len = shim->persist_blob_len < capacity ? shim->persist_blob_len : capacity;
  memcpy(out, shim->persist_blob, len);
  return len;
}

uint32_t dctp_shim_manifest_crc32(const dctp_shim_t *shim) {
  return dctp_device_manifest_crc32(&shim->device);
}

uint32_t dctp_shim_storage_generation(const dctp_shim_t *shim) {
  return dctp_device_storage_generation(&shim->device);
}

int dctp_shim_session_active(const dctp_shim_t *shim) {
  return dctp_device_session_active(&shim->device) ? 1 : 0;
}

int dctp_shim_log(dctp_shim_t *shim, uint8_t severity, uint16_t module_id, uint32_t timestamp_us,
                  const char *text) {
  return dctp_device_log(&shim->device, severity, module_id, timestamp_us, text) ? 1 : 0;
}

int dctp_shim_set_value_f32(dctp_shim_t *shim, uint32_t param_id, float value) {
  dctp_value_t wrapped;
  wrapped.type = DCTP_TYPE_F32;
  wrapped.as.f32 = value;
  return dctp_device_set_value(&shim->device, param_id, wrapped) ? 1 : 0;
}

int dctp_shim_get_value_bits(const dctp_shim_t *shim, uint32_t param_id, uint8_t *type_out, uint32_t *bits_out) {
  dctp_value_t value;
  if (!dctp_device_get_value(&shim->device, param_id, &value)) {
    return 0;
  }
  *type_out = value.type;
  switch (value.type) {
    case DCTP_TYPE_I32:
      *bits_out = (uint32_t)value.as.i32;
      break;
    case DCTP_TYPE_U32:
      *bits_out = value.as.u32;
      break;
    case DCTP_TYPE_F32:
      *bits_out = dctp_f32_to_bits(value.as.f32);
      break;
    case DCTP_TYPE_BOOL:
      *bits_out = value.as.boolean;
      break;
    case DCTP_TYPE_ENUM:
      *bits_out = (uint32_t)value.as.enum_value;
      break;
    default:
      return 0;
  }
  return 1;
}

int dctp_shim_storage_apply(dctp_shim_t *shim, const uint8_t *slot_a, uint32_t len_a, const uint8_t *slot_b,
                            uint32_t len_b) {
  return dctp_storage_apply(&shim->device, slot_a, len_a, slot_b, len_b) ? 1 : 0;
}

/* ------------------------------------------------------------------ */
/* 黄金向量构造：与 generate_vectors.rs 相同的固定输入 */

typedef struct {
  uint8_t *out;
  size_t capacity;
  size_t len;
} capture_t;

static void capture_write(void *user, const uint8_t *bytes, size_t len) {
  capture_t *capture = (capture_t *)user;
  if (capture->capacity - capture->len >= len) {
    memcpy(capture->out + capture->len, bytes, len);
    capture->len += len;
  }
}

static size_t emit_golden_frame(uint8_t message_type, uint8_t flags, uint16_t sequence, uint32_t session_id,
                                const dctp_writer_t *payload_writer, uint8_t *raw, uint8_t *out, size_t capacity) {
  size_t total = dctp_frame_finalize(raw, message_type, flags, sequence, session_id, (uint16_t)payload_writer->len);
  if (total == 0) {
    return 0;
  }
  capture_t capture = {out, capacity, 0};
  dctp_cobs_send(raw, total, capture_write, &capture);
  return capture.len;
}

size_t dctp_shim_build_golden(int which, uint8_t *out, size_t capacity) {
  uint8_t raw[DCTP_RAW_FRAME_MAX];
  dctp_writer_t writer;
  dctp_writer_init(&writer, raw + DCTP_HEADER_LEN, DCTP_MAX_PAYLOAD);

  switch (which) {
    case 0: /* hello.bin */
      dctp_put_u32(&writer, 0x10203040u);
      dctp_put_u8(&writer, 1);
      dctp_put_u8(&writer, 1);
      dctp_put_u16(&writer, 1024);
      return emit_golden_frame(DCTP_MSG_HELLO, DCTP_FLAG_ACK_REQUIRED, 0x1001, 0, &writer, raw, out, capacity);
    case 1: /* hello-ack.bin */
      dctp_put_u32(&writer, 0xA1B2C3D4u);
      dctp_put_bytes(&writer, (const uint8_t *)"DCTP-VECTOR-0001", 16);
      dctp_put_u32(&writer, 7);
      dctp_put_u16(&writer, 1);
      dctp_put_u16(&writer, 2);
      dctp_put_u16(&writer, 3);
      dctp_put_u16(&writer, 1);
      dctp_put_u16(&writer, 0);
      dctp_put_u16(&writer, 0);
      dctp_put_u32(&writer, DCTP_CAP_PARAMETERS | DCTP_CAP_TELEMETRY);
      dctp_put_u32(&writer, 0x89ABCDEFu);
      dctp_put_u16(&writer, 1024);
      return emit_golden_frame(DCTP_MSG_HELLO_ACK, DCTP_FLAG_RESPONSE, 0x1001, 0xA1B2C3D4u, &writer, raw, out,
                               capacity);
    case 2: /* param-write.bin */
      dctp_put_u32(&writer, 1);
      dctp_put_u32(&writer, 7);
      dctp_put_u8(&writer, DCTP_TYPE_F32);
      dctp_put_u32(&writer, dctp_f32_to_bits(2.5f));
      return emit_golden_frame(DCTP_MSG_PARAM_WRITE, DCTP_FLAG_ACK_REQUIRED, 0x1003, 0xA1B2C3D4u, &writer, raw,
                               out, capacity);
    case 3: /* param-value.bin */
      dctp_put_u32(&writer, 1);
      dctp_put_u32(&writer, 7);
      dctp_put_u8(&writer, DCTP_TYPE_F32);
      dctp_put_u32(&writer, dctp_f32_to_bits(2.5f));
      dctp_put_u8(&writer, 1);
      dctp_put_u8(&writer, DCTP_TYPE_F32);
      dctp_put_u32(&writer, dctp_f32_to_bits(1.0f));
      return emit_golden_frame(DCTP_MSG_PARAM_VALUE, DCTP_FLAG_RESPONSE, 0x1003, 0xA1B2C3D4u, &writer, raw, out,
                               capacity);
    case 4: /* param-commit-ack.bin */
      dctp_put_u32(&writer, 0x12345678u);
      dctp_put_u32(&writer, 42);
      return emit_golden_frame(DCTP_MSG_PARAM_COMMIT_ACK, DCTP_FLAG_RESPONSE, 0x1003, 0xA1B2C3D4u, &writer, raw,
                               out, capacity);
    case 5: /* telemetry-mixed.bin */
      dctp_put_u16(&writer, 7);
      dctp_put_u16(&writer, 42);
      dctp_put_u8(&writer, 2);
      dctp_put_u8(&writer, 4);
      dctp_put_u16(&writer, 3);
      dctp_put_u32(&writer, 0x11223344u);
      dctp_put_u16(&writer, 0);
      dctp_put_u32(&writer, dctp_f32_to_bits(1.5f));
      dctp_put_u32(&writer, (uint32_t)(-4));
      dctp_put_u32(&writer, 8);
      dctp_put_u32(&writer, 0x5u);
      dctp_put_u16(&writer, 2000);
      dctp_put_u32(&writer, dctp_f32_to_bits(1.75f));
      dctp_put_u32(&writer, (uint32_t)(-3));
      dctp_put_u32(&writer, 9);
      dctp_put_u32(&writer, 0x1u);
      return emit_golden_frame(DCTP_MSG_TELEMETRY_DATA, 0, 0x1004, 0xA1B2C3D4u, &writer, raw, out, capacity);
    default:
      return 0;
  }
}
