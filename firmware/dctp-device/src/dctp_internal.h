/* 库内部共享定义：wire 原语、CRC、COBS 发送与消息常量。不属于公共 API。 */
#ifndef DCTP_INTERNAL_H
#define DCTP_INTERNAL_H

#include "dctp_device.h"

#define DCTP_MAGIC 0x5444u

enum {
  DCTP_MSG_HELLO = 0x01,
  DCTP_MSG_HELLO_ACK = 0x02,
  DCTP_MSG_HEARTBEAT = 0x03,
  DCTP_MSG_HEARTBEAT_ACK = 0x04,
  DCTP_MSG_SESSION_CLOSE = 0x05,
  DCTP_MSG_MANIFEST_REQUEST = 0x10,
  DCTP_MSG_MANIFEST_CHUNK = 0x11,
  DCTP_MSG_MANIFEST_DONE = 0x12,
  DCTP_MSG_PARAM_READ = 0x20,
  DCTP_MSG_PARAM_VALUE = 0x21,
  DCTP_MSG_PARAM_WRITE = 0x22,
  DCTP_MSG_PARAM_WRITE_ACK = 0x23,
  DCTP_MSG_PARAM_COMMIT = 0x24,
  DCTP_MSG_PARAM_COMMIT_ACK = 0x25,
  DCTP_MSG_TELEMETRY_SUBSCRIBE = 0x30,
  DCTP_MSG_TELEMETRY_SUBSCRIBE_ACK = 0x31,
  DCTP_MSG_TELEMETRY_DATA = 0x32,
  DCTP_MSG_TELEMETRY_STOP = 0x33,
  DCTP_MSG_LOG_MESSAGE = 0x40,
  DCTP_MSG_DEVICE_EVENT = 0x41,
  DCTP_MSG_PREPARE_FLASH = 0x50,
  DCTP_MSG_PREPARE_FLASH_ACK = 0x51,
  DCTP_MSG_ERROR = 0x7f,
};

enum {
  DCTP_FLAG_ACK_REQUIRED = 1u << 0,
  DCTP_FLAG_RESPONSE = 1u << 1,
  DCTP_FLAG_ERROR = 1u << 2,
  DCTP_FLAG_MORE_FRAGMENTS = 1u << 3,
};

enum {
  DCTP_ERR_UNSUPPORTED_VERSION = 1,
  DCTP_ERR_INVALID_SESSION = 2,
  DCTP_ERR_UNKNOWN_MESSAGE = 3,
  DCTP_ERR_INVALID_LENGTH = 4,
  DCTP_ERR_INVALID_PARAM_ID = 5,
  DCTP_ERR_TYPE_MISMATCH = 6,
  DCTP_ERR_OUT_OF_RANGE = 7,
  DCTP_ERR_READ_ONLY = 8,
  DCTP_ERR_REVISION_CONFLICT = 9,
  DCTP_ERR_BUSY = 10,
  DCTP_ERR_QUEUE_FULL = 11,
  DCTP_ERR_STORAGE_FAILED = 12,
  DCTP_ERR_VERIFY_FAILED = 13,
  DCTP_ERR_NOT_READY = 14,
  DCTP_ERR_INTERNAL_ERROR = 15,
};

enum {
  DCTP_CAP_PARAMETERS = 1u << 0,
  DCTP_CAP_TELEMETRY = 1u << 1,
  DCTP_CAP_PERSISTENCE = 1u << 2,
  DCTP_CAP_STRUCTURED_LOG = 1u << 3,
  DCTP_CAP_PREPARE_FLASH = 1u << 4,
};

#define DCTP_HELLO_PAYLOAD_LEN 8u
#define DCTP_HELLO_ACK_PAYLOAD_LEN 46u
#define DCTP_MANIFEST_CHUNK_PREFIX_LEN 12u
#define DCTP_TELEMETRY_BATCH_PREFIX_LEN 12u
#define DCTP_FIRMWARE_FLASH_SCHEMA_VERSION 1u
#define DCTP_PREPARE_FLASH_PAYLOAD_LEN 63u
#define DCTP_PREPARE_FLASH_ACK_PAYLOAD_LEN 24u
#define DCTP_SESSION_EXPIRATION_MS 3000u
#define DCTP_MAX_ERROR_CONTEXT_LEN 64u
#define DCTP_STORAGE_MAGIC 0x31565044u /* "DPV1" 小端 */
#define DCTP_STORAGE_VERSION 1u

/* 有界小端写入游标；溢出后 ok 置 false 且后续写入被忽略。 */
typedef struct {
  uint8_t *bytes;
  size_t capacity;
  size_t len;
  bool ok;
} dctp_writer_t;

void dctp_writer_init(dctp_writer_t *writer, uint8_t *bytes, size_t capacity);
void dctp_put_u8(dctp_writer_t *writer, uint8_t value);
void dctp_put_u16(dctp_writer_t *writer, uint16_t value);
void dctp_put_u32(dctp_writer_t *writer, uint32_t value);
void dctp_put_bytes(dctp_writer_t *writer, const uint8_t *bytes, size_t len);
void dctp_put_str_u8_len(dctp_writer_t *writer, const char *value);

/* 有界小端读取游标；越界后 ok 置 false 且后续读取返回 0。 */
typedef struct {
  const uint8_t *bytes;
  size_t len;
  size_t offset;
  bool ok;
} dctp_reader_t;

void dctp_reader_init(dctp_reader_t *reader, const uint8_t *bytes, size_t len);
uint8_t dctp_read_u8(dctp_reader_t *reader);
uint16_t dctp_read_u16(dctp_reader_t *reader);
uint32_t dctp_read_u32(dctp_reader_t *reader);
bool dctp_reader_done(const dctp_reader_t *reader);

uint16_t dctp_crc16_ccitt_false(const uint8_t *bytes, size_t len);
uint32_t dctp_crc32_update(uint32_t state, const uint8_t *bytes, size_t len);
uint32_t dctp_crc32_finish(uint32_t state);
uint32_t dctp_crc32(const uint8_t *bytes, size_t len);
#define DCTP_CRC32_INIT 0xFFFFFFFFu

float dctp_bits_to_f32(uint32_t bits);
uint32_t dctp_f32_to_bits(float value);

/*
 * 在 raw 缓冲（容量 DCTP_RAW_FRAME_MAX）就地组装帧头 + payload + CRC16。
 * payload 必须已写在 raw + DCTP_HEADER_LEN 处。返回整帧长度，失败返回 0。
 */
size_t dctp_frame_finalize(uint8_t *raw, uint8_t message_type, uint8_t flags, uint16_t sequence,
                           uint32_t session_id, uint16_t payload_len);

/* COBS 编码 raw[0..len) 并追加 0x00 定界符，经 write 回调分块发出。 */
void dctp_cobs_send(const uint8_t *raw, size_t len, void (*write)(void *user, const uint8_t *bytes, size_t len),
                    void *user);

/* COBS 编码后线上长度的上界（含结尾 0x00），用于 tx_free 预算判断。 */
size_t dctp_cobs_encoded_len(size_t raw_len);

#endif /* DCTP_INTERNAL_H */
