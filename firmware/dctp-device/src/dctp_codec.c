/* wire 原语、CRC 与 COBS 分帧。逐字节对齐 crates/dctp-protocol 的实现。 */
#include <string.h>

#include "dctp_internal.h"

void dctp_writer_init(dctp_writer_t *writer, uint8_t *bytes, size_t capacity) {
  writer->bytes = bytes;
  writer->capacity = capacity;
  writer->len = 0;
  writer->ok = true;
}

static bool writer_reserve(dctp_writer_t *writer, size_t extra) {
  if (!writer->ok || writer->capacity - writer->len < extra) {
    writer->ok = false;
    return false;
  }
  return true;
}

void dctp_put_u8(dctp_writer_t *writer, uint8_t value) {
  if (!writer_reserve(writer, 1)) {
    return;
  }
  writer->bytes[writer->len++] = value;
}

void dctp_put_u16(dctp_writer_t *writer, uint16_t value) {
  if (!writer_reserve(writer, 2)) {
    return;
  }
  writer->bytes[writer->len++] = (uint8_t)(value & 0xFFu);
  writer->bytes[writer->len++] = (uint8_t)(value >> 8);
}

void dctp_put_u32(dctp_writer_t *writer, uint32_t value) {
  if (!writer_reserve(writer, 4)) {
    return;
  }
  writer->bytes[writer->len++] = (uint8_t)(value & 0xFFu);
  writer->bytes[writer->len++] = (uint8_t)((value >> 8) & 0xFFu);
  writer->bytes[writer->len++] = (uint8_t)((value >> 16) & 0xFFu);
  writer->bytes[writer->len++] = (uint8_t)(value >> 24);
}

void dctp_put_bytes(dctp_writer_t *writer, const uint8_t *bytes, size_t len) {
  if (!writer_reserve(writer, len)) {
    return;
  }
  memcpy(writer->bytes + writer->len, bytes, len);
  writer->len += len;
}

void dctp_put_str_u8_len(dctp_writer_t *writer, const char *value) {
  size_t len = value != NULL ? strlen(value) : 0;
  if (len > 255u) {
    writer->ok = false;
    return;
  }
  dctp_put_u8(writer, (uint8_t)len);
  dctp_put_bytes(writer, (const uint8_t *)value, len);
}

void dctp_reader_init(dctp_reader_t *reader, const uint8_t *bytes, size_t len) {
  reader->bytes = bytes;
  reader->len = len;
  reader->offset = 0;
  reader->ok = true;
}

static bool reader_take(dctp_reader_t *reader, size_t len) {
  if (!reader->ok || reader->len - reader->offset < len) {
    reader->ok = false;
    return false;
  }
  return true;
}

uint8_t dctp_read_u8(dctp_reader_t *reader) {
  if (!reader_take(reader, 1)) {
    return 0;
  }
  return reader->bytes[reader->offset++];
}

uint16_t dctp_read_u16(dctp_reader_t *reader) {
  if (!reader_take(reader, 2)) {
    return 0;
  }
  uint16_t value = (uint16_t)(reader->bytes[reader->offset] | ((uint16_t)reader->bytes[reader->offset + 1] << 8));
  reader->offset += 2;
  return value;
}

uint32_t dctp_read_u32(dctp_reader_t *reader) {
  if (!reader_take(reader, 4)) {
    return 0;
  }
  uint32_t value = (uint32_t)reader->bytes[reader->offset] |
                   ((uint32_t)reader->bytes[reader->offset + 1] << 8) |
                   ((uint32_t)reader->bytes[reader->offset + 2] << 16) |
                   ((uint32_t)reader->bytes[reader->offset + 3] << 24);
  reader->offset += 4;
  return value;
}

bool dctp_reader_done(const dctp_reader_t *reader) {
  return reader->ok && reader->offset == reader->len;
}

uint16_t dctp_crc16_ccitt_false(const uint8_t *bytes, size_t len) {
  uint16_t crc = 0xFFFFu;
  for (size_t index = 0; index < len; index += 1) {
    crc ^= (uint16_t)((uint16_t)bytes[index] << 8);
    for (int bit = 0; bit < 8; bit += 1) {
      crc = (uint16_t)((crc & 0x8000u) != 0 ? (uint16_t)(crc << 1) ^ 0x1021u : (uint16_t)(crc << 1));
    }
  }
  return crc;
}

uint32_t dctp_crc32_update(uint32_t state, const uint8_t *bytes, size_t len) {
  for (size_t index = 0; index < len; index += 1) {
    state ^= bytes[index];
    for (int bit = 0; bit < 8; bit += 1) {
      state = (state & 1u) != 0 ? (state >> 1) ^ 0xEDB88320u : state >> 1;
    }
  }
  return state;
}

uint32_t dctp_crc32_finish(uint32_t state) {
  return state ^ 0xFFFFFFFFu;
}

uint32_t dctp_crc32(const uint8_t *bytes, size_t len) {
  return dctp_crc32_finish(dctp_crc32_update(DCTP_CRC32_INIT, bytes, len));
}

float dctp_bits_to_f32(uint32_t bits) {
  float value;
  memcpy(&value, &bits, sizeof value);
  return value;
}

uint32_t dctp_f32_to_bits(float value) {
  uint32_t bits;
  memcpy(&bits, &value, sizeof bits);
  return bits;
}

size_t dctp_frame_finalize(uint8_t *raw, uint8_t message_type, uint8_t flags, uint16_t sequence,
                           uint32_t session_id, uint16_t payload_len) {
  if (payload_len > DCTP_MAX_PAYLOAD) {
    return 0;
  }
  dctp_writer_t writer;
  dctp_writer_init(&writer, raw, DCTP_HEADER_LEN);
  dctp_put_u16(&writer, DCTP_MAGIC);
  dctp_put_u8(&writer, DCTP_PROTOCOL_VERSION);
  dctp_put_u8(&writer, message_type);
  dctp_put_u8(&writer, flags);
  dctp_put_u16(&writer, sequence);
  dctp_put_u32(&writer, session_id);
  dctp_put_u16(&writer, payload_len);
  if (!writer.ok || writer.len != DCTP_HEADER_LEN) {
    return 0;
  }
  size_t crc_offset = DCTP_HEADER_LEN + payload_len;
  uint16_t crc = dctp_crc16_ccitt_false(raw, crc_offset);
  raw[crc_offset] = (uint8_t)(crc & 0xFFu);
  raw[crc_offset + 1] = (uint8_t)(crc >> 8);
  return crc_offset + 2;
}

void dctp_cobs_send(const uint8_t *raw, size_t len, void (*write)(void *user, const uint8_t *bytes, size_t len),
                    void *user) {
  /* 分块流式输出，与 dctp-protocol 的 cobs_encode 产出逐字节一致。 */
  uint8_t chunk[256];
  size_t chunk_len = 1;
  uint8_t code = 1;

  for (size_t index = 0; index < len; index += 1) {
    uint8_t byte = raw[index];
    if (byte == 0) {
      chunk[0] = code;
      write(user, chunk, chunk_len);
      chunk_len = 1;
      code = 1;
    } else {
      chunk[chunk_len++] = byte;
      code += 1;
      if (code == 0xFFu) {
        chunk[0] = code;
        write(user, chunk, chunk_len);
        chunk_len = 1;
        code = 1;
      }
    }
  }

  chunk[0] = code;
  write(user, chunk, chunk_len);
  uint8_t delimiter = 0;
  write(user, &delimiter, 1);
}

size_t dctp_cobs_encoded_len(size_t raw_len) {
  /* 上界（含结尾 0x00），用于发送预算判断，与实际编码长度最多差数字节。 */
  return raw_len + (raw_len + 253u) / 254u + 1u;
}
