/*
 * DiCar Tune 车端 DCTP v1 参考库
 *
 * 纯 C99、零动态分配、无操作系统依赖。把本目录的 include/ 与 src/ 加入
 * 工程即可移植；平台相关的串口发送、Flash 写入与时钟由回调注入。
 * 完整移植说明见同目录 README.md。
 *
 * 线上格式与行为以仓库 crates/dctp-protocol 与 crates/dctp-sim 为权威，
 * crates/dctp-device-c 在每次 cargo test 时对本实现做逐字节交叉校验。
 */
#ifndef DCTP_DEVICE_H
#define DCTP_DEVICE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------ */
/* 编译期资源上限（可在编译选项中覆盖，决定 dctp_device_t 的大小） */

#ifndef DCTP_MAX_PARAMS
#define DCTP_MAX_PARAMS 64
#endif

#ifndef DCTP_MAX_CHANNELS
#define DCTP_MAX_CHANNELS 16
#endif

/* 解码后 Payload 上限；协议允许 1-1024，实际会话取 HELLO 协商的较小值。 */
#ifndef DCTP_MAX_PAYLOAD
#define DCTP_MAX_PAYLOAD 1024
#endif

/* 规格要求至少缓存最近 32 个可靠请求以保证重试幂等。 */
#ifndef DCTP_REQUEST_CACHE_ENTRIES
#define DCTP_REQUEST_CACHE_ENTRIES 32
#endif

#define DCTP_PROTOCOL_VERSION 1u
#define DCTP_HEADER_LEN 13u
#define DCTP_RAW_FRAME_MAX (DCTP_HEADER_LEN + DCTP_MAX_PAYLOAD + 2u)
#define DCTP_DEVICE_ID_LEN 16u
#define DCTP_TELEMETRY_MAX_SUBSCRIBED 8u
#define DCTP_TELEMETRY_MAX_SAMPLES 16u
#define DCTP_CACHE_PAYLOAD_MAX 72u
#define DCTP_STORAGE_BLOB_MAX (24u + (uint32_t)DCTP_MAX_PARAMS * 9u + 4u)

/* ------------------------------------------------------------------ */
/* 参数与遥测描述 */

typedef enum {
  DCTP_TYPE_I32 = 1,
  DCTP_TYPE_U32 = 2,
  DCTP_TYPE_F32 = 3,
  DCTP_TYPE_BOOL = 4,
  DCTP_TYPE_ENUM = 5,
} dctp_param_type_t;

typedef enum {
  DCTP_TELEMETRY_F32 = 1,
  DCTP_TELEMETRY_I32 = 2,
  DCTP_TELEMETRY_U32 = 3,
  DCTP_TELEMETRY_FLAGS32 = 4,
} dctp_telemetry_type_t;

enum {
  DCTP_PARAM_WRITABLE = 1u << 0,
  DCTP_PARAM_PERSISTENT = 1u << 1,
  DCTP_PARAM_DANGEROUS = 1u << 2,
};

typedef struct {
  uint8_t type; /* dctp_param_type_t */
  union {
    int32_t i32;
    uint32_t u32;
    float f32;
    uint8_t boolean;    /* 0 或 1 */
    int32_t enum_value; /* 枚举取选项 value */
  } as;
} dctp_value_t;

typedef struct {
  int32_t value;
  const char *label; /* UTF-8，最长 32 字节 */
} dctp_enum_option_t;

enum {
  DCTP_CONSTRAINT_NONE = 0,
  DCTP_CONSTRAINT_NUMERIC = 1,
  DCTP_CONSTRAINT_ENUM = 2,
};

/*
 * 参数描述表由固件以 const 静态数组提供，param_id 必须严格升序且唯一。
 * 字符串为 UTF-8：machine_name<=48、display_name<=64、group<=32、unit<=16 字节。
 */
typedef struct {
  uint32_t param_id;
  uint8_t type;  /* dctp_param_type_t */
  uint8_t flags; /* DCTP_PARAM_* 位 */
  const char *machine_name;
  const char *display_name;
  const char *group;
  const char *unit;
  dctp_value_t default_value;
  uint8_t constraint_kind; /* DCTP_CONSTRAINT_* */
  dctp_value_t min;        /* NUMERIC 时有效，类型须与参数一致 */
  dctp_value_t max;
  dctp_value_t step;
  const dctp_enum_option_t *enum_options; /* ENUM 时有效 */
  uint8_t enum_option_count;              /* 最多 32 */
} dctp_param_descriptor_t;

/*
 * 遥测通道表同样为 const 静态数组，channel_id 严格升序且唯一。
 * read 回调在 poll 内按订阅采样率调用，必须快速无阻塞，返回 32 位线上
 * 位型：f32 用位模式（memcpy），i32 强转 u32，flags32 原样。
 */
typedef struct {
  uint32_t channel_id;
  uint8_t type; /* dctp_telemetry_type_t */
  const char *machine_name;
  const char *display_name;
  const char *group;
  const char *unit;
  uint32_t (*read)(void *user);
} dctp_channel_descriptor_t;

/* ------------------------------------------------------------------ */
/* 平台回调 */

enum {
  DCTP_PERSIST_OK = 0,
  DCTP_PERSIST_STORAGE_FAILED = 1, /* 写入失败 -> STORAGE_FAILED */
  DCTP_PERSIST_VERIFY_FAILED = 2,  /* 读回校验失败 -> VERIFY_FAILED */
};

typedef struct {
  const dctp_param_descriptor_t *params;
  uint16_t param_count;
  const dctp_channel_descriptor_t *channels;
  uint16_t channel_count;

  uint8_t device_id[DCTP_DEVICE_ID_LEN];
  uint32_t boot_count;
  uint16_t firmware_major;
  uint16_t firmware_minor;
  uint16_t firmware_patch;

  /*
   * 必填。把 len 字节写入 UART 发送路径。可靠响应经此发出，移植层必须
   * 保证其被完整接受（足够大的发送环形缓冲，或在此阻塞到放完）。
   */
  void (*write)(void *user, const uint8_t *bytes, size_t len);

  /*
   * 可选（NULL 表示不限制）。返回发送路径当前可接受的字节数。遥测批次
   * 与日志发送前查询；空间不足时整帧丢弃并累计丢弃计数，绝不发半帧。
   */
  size_t (*tx_free)(void *user);

  /*
   * 可选（NULL 表示无持久化能力，PARAM_COMMIT 返回 READ_ONLY）。
   * 把 blob 写入非活动槽并读回校验，返回 DCTP_PERSIST_*。只有返回
   * DCTP_PERSIST_OK 后库才更新 Flash 影子值并递增 Generation。
   */
  int (*persist)(void *user, const uint8_t *blob, uint32_t len);

  void *user;
} dctp_device_config_t;

/* ------------------------------------------------------------------ */
/* 设备实例。字段全部私有，只为允许静态分配而公开布局。 */

typedef struct {
  uint32_t session_id;
  uint8_t request_type;
  uint16_t sequence;
  uint8_t response_type;
  uint8_t response_flags;
  uint8_t payload_len;
  uint8_t payload[DCTP_CACHE_PAYLOAD_MAX];
} dctp_cache_entry_t;

typedef struct {
  dctp_value_t value;
  dctp_value_t persisted_value;
  uint32_t revision;
  uint8_t has_persisted;
} dctp_param_state_t;

typedef struct {
  uint16_t subscription_version;
  uint16_t sample_rate_hz;
  uint8_t channel_count;
  uint8_t channel_index[DCTP_TELEMETRY_MAX_SUBSCRIBED]; /* 指向通道表下标 */
} dctp_subscription_t;

typedef struct {
  dctp_device_config_t config;

  /* 会话 */
  bool session_active;
  uint32_t session_id;
  uint32_t session_last_valid_ms;
  uint16_t session_max_payload;
  uint32_t session_counter;

  /* Manifest（init 时预计算） */
  uint32_t manifest_crc32;
  uint32_t manifest_total_len;

  /* 参数运行状态，与描述表下标一一对应 */
  dctp_param_state_t param_state[DCTP_MAX_PARAMS];
  uint32_t storage_generation;

  /* 可靠请求幂等缓存（FIFO 覆盖） */
  dctp_cache_entry_t cache[DCTP_REQUEST_CACHE_ENTRIES];
  uint8_t cache_len;
  uint8_t cache_next;

  /* HELLO / SESSION_CLOSE 重放 */
  bool hello_completed;
  uint16_t hello_sequence;
  uint8_t hello_flags;
  uint8_t hello_request[8];
  uint8_t hello_response[46];
  bool close_completed;
  uint16_t close_sequence;
  uint8_t close_flags;
  uint32_t close_session_id;

  /* 遥测 */
  bool telemetry_active;
  bool telemetry_pending_start;
  dctp_subscription_t subscription;
  uint32_t next_telemetry_at_us;
  uint16_t next_telemetry_sequence;
  uint16_t pending_dropped_samples;
  uint16_t log_sequence;
  uint32_t dropped_telemetry_frames;
  uint32_t dropped_log_messages;

  /* COBS 流解码器（渐进解码，收满一帧立即处理） */
  uint8_t rx_frame[DCTP_RAW_FRAME_MAX];
  uint16_t rx_len;
  uint8_t rx_block_remaining;
  bool rx_block_append_zero;
  bool rx_dropping;
  bool rx_started;
  uint32_t rx_malformed_frames;

  /* 发送与持久化的组帧缓冲 */
  uint8_t tx_raw[DCTP_RAW_FRAME_MAX];
  uint8_t storage_blob[DCTP_STORAGE_BLOB_MAX];
} dctp_device_t;

/* ------------------------------------------------------------------ */
/* API */

/*
 * 校验描述表（排序、唯一、字符串长度、约束类型、枚举去重）并预计算
 * Manifest CRC32。表不合法时返回 false，设备不可用。
 */
bool dctp_device_init(dctp_device_t *device, const dctp_device_config_t *config);

/*
 * 喂入串口收到的字节并在主循环上下文处理完整帧（响应经 write 回调发出）。
 * 不可在中断里调用；ISR 应只把字节放入自己的环形缓冲。
 * now_ms 为单调毫秒时钟，用于会话 3000 ms 失效判断。
 */
void dctp_device_rx(dctp_device_t *device, const uint8_t *bytes, size_t len, uint32_t now_ms);

/*
 * 周期调用（建议 >= 2x 遥测采样率，至少每 16 ms 一次）：推进会话失效
 * 与遥测批次。now_us 为单调微秒时钟，允许自然回绕。
 */
void dctp_device_poll(dctp_device_t *device, uint32_t now_ms, uint32_t now_us);

/*
 * 发送结构化日志（P3：发送空间不足时静默丢弃并计数）。text 为 UTF-8，
 * 最长 192 字节。无活动会话时返回 false。
 */
bool dctp_device_log(dctp_device_t *device, uint8_t severity, uint16_t module_id,
                     uint32_t timestamp_us, const char *text);

/* 固件内部读写参数。set 会校验类型与约束并递增 value_revision。 */
bool dctp_device_get_value(const dctp_device_t *device, uint32_t param_id, dctp_value_t *out);
bool dctp_device_set_value(dctp_device_t *device, uint32_t param_id, dctp_value_t value);

/*
 * 启动时从 A/B 双槽恢复固化参数：传入两个槽的原始字节（无效槽可传
 * NULL/0），库选择 CRC 合法且 Generation 较新的槽，把其中与当前描述表
 * id 和类型都匹配的值应用为 RAM 值与 Flash 影子值。返回是否有槽生效。
 */
bool dctp_storage_apply(dctp_device_t *device, const uint8_t *slot_a, uint32_t len_a,
                        const uint8_t *slot_b, uint32_t len_b);

uint32_t dctp_device_storage_generation(const dctp_device_t *device);
bool dctp_device_session_active(const dctp_device_t *device);
uint32_t dctp_device_manifest_crc32(const dctp_device_t *device);

#ifdef __cplusplus
}
#endif

#endif /* DCTP_DEVICE_H */
