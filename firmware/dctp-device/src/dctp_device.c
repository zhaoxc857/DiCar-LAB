/*
 * DCTP v1 设备侧状态机。
 *
 * 行为与 crates/dctp-sim 的 SimDevice 逐条对齐：会话建立与 3000 ms 失效、
 * HELLO/SESSION_CLOSE 重放、可靠请求幂等缓存、参数校验链与 Revision 冲突
 * 上下文、Commit 的 canonical CRC 与单次递增 Generation、Manifest 分片以及
 * 遥测批次的节拍、容量与丢弃计数。
 */
#include <string.h>

#include "dctp_internal.h"

#define DCTP_MANIFEST_SCHEMA_VERSION 1u
#define DCTP_MANIFEST_MAX_LEN (64u * 1024u)
#define DCTP_PROTOCOL_MAX_PARAMS 64u
#define DCTP_PROTOCOL_MAX_CHANNELS 16u
#define DCTP_MAX_LOG_TEXT_LEN 192u
#define DCTP_SDK_VERSION_MAJOR 0u
#define DCTP_SDK_VERSION_MINOR 1u
#define DCTP_SDK_VERSION_PATCH 0u

typedef struct {
  uint8_t type;
  uint8_t flags;
  uint16_t sequence;
  uint32_t session_id;
  const uint8_t *payload;
  uint16_t payload_len;
  bool reliable;
} dctp_request_t;

/* ------------------------------------------------------------------ */
/* 查表与取值 */

static int find_param(const dctp_device_t *device, uint32_t param_id) {
  for (uint16_t index = 0; index < device->config.param_count; index += 1) {
    if (device->config.params[index].param_id == param_id) {
      return (int)index;
    }
  }
  return -1;
}

static int find_channel(const dctp_device_t *device, uint32_t channel_id) {
  for (uint16_t index = 0; index < device->config.channel_count; index += 1) {
    if (device->config.channels[index].channel_id == channel_id) {
      return (int)index;
    }
  }
  return -1;
}

static size_t tagged_value_len(uint8_t type) {
  return type == DCTP_TYPE_BOOL ? 2u : 5u;
}

static void put_tagged_value(dctp_writer_t *writer, const dctp_value_t *value) {
  dctp_put_u8(writer, value->type);
  switch (value->type) {
    case DCTP_TYPE_I32:
      dctp_put_u32(writer, (uint32_t)value->as.i32);
      break;
    case DCTP_TYPE_U32:
      dctp_put_u32(writer, value->as.u32);
      break;
    case DCTP_TYPE_F32:
      dctp_put_u32(writer, dctp_f32_to_bits(value->as.f32));
      break;
    case DCTP_TYPE_BOOL:
      dctp_put_u8(writer, value->as.boolean != 0 ? 1u : 0u);
      break;
    case DCTP_TYPE_ENUM:
      dctp_put_u32(writer, (uint32_t)value->as.enum_value);
      break;
    default:
      writer->ok = false;
      break;
  }
}

static bool read_tagged_value(dctp_reader_t *reader, dctp_value_t *value) {
  uint8_t type = dctp_read_u8(reader);
  value->type = type;
  switch (type) {
    case DCTP_TYPE_I32:
      value->as.i32 = (int32_t)dctp_read_u32(reader);
      break;
    case DCTP_TYPE_U32:
      value->as.u32 = dctp_read_u32(reader);
      break;
    case DCTP_TYPE_F32:
      value->as.f32 = dctp_bits_to_f32(dctp_read_u32(reader));
      break;
    case DCTP_TYPE_BOOL: {
      uint8_t raw = dctp_read_u8(reader);
      if (raw > 1u) {
        return false;
      }
      value->as.boolean = raw;
      break;
    }
    case DCTP_TYPE_ENUM:
      value->as.enum_value = (int32_t)dctp_read_u32(reader);
      break;
    default:
      return false;
  }
  return reader->ok;
}

/* Commit 的 canonical 输入：param_id + type + 小端值字节。 */
static uint32_t canonical_crc_update(uint32_t state, uint32_t param_id, const dctp_value_t *value) {
  uint8_t scratch[9];
  dctp_writer_t writer;
  dctp_writer_init(&writer, scratch, sizeof scratch);
  dctp_put_u32(&writer, param_id);
  dctp_put_u8(&writer, value->type);
  switch (value->type) {
    case DCTP_TYPE_I32:
      dctp_put_u32(&writer, (uint32_t)value->as.i32);
      break;
    case DCTP_TYPE_U32:
      dctp_put_u32(&writer, value->as.u32);
      break;
    case DCTP_TYPE_F32:
      dctp_put_u32(&writer, dctp_f32_to_bits(value->as.f32));
      break;
    case DCTP_TYPE_BOOL:
      dctp_put_u8(&writer, value->as.boolean != 0 ? 1u : 0u);
      break;
    case DCTP_TYPE_ENUM:
      dctp_put_u32(&writer, (uint32_t)value->as.enum_value);
      break;
    default:
      break;
  }
  return dctp_crc32_update(state, scratch, writer.len);
}

static bool value_within_constraints(const dctp_value_t *value, const dctp_param_descriptor_t *descriptor) {
  switch (descriptor->constraint_kind) {
    case DCTP_CONSTRAINT_NONE:
      return true;
    case DCTP_CONSTRAINT_NUMERIC:
      switch (value->type) {
        case DCTP_TYPE_I32:
          return value->as.i32 >= descriptor->min.as.i32 && value->as.i32 <= descriptor->max.as.i32;
        case DCTP_TYPE_U32:
          return value->as.u32 >= descriptor->min.as.u32 && value->as.u32 <= descriptor->max.as.u32;
        case DCTP_TYPE_F32:
          /* NaN 与无穷不可写入。 */
          return value->as.f32 >= descriptor->min.as.f32 && value->as.f32 <= descriptor->max.as.f32 &&
                 (value->as.f32 - value->as.f32) == 0.0f;
        default:
          return false;
      }
    case DCTP_CONSTRAINT_ENUM:
      if (value->type != DCTP_TYPE_ENUM) {
        return false;
      }
      for (uint8_t index = 0; index < descriptor->enum_option_count; index += 1) {
        if (descriptor->enum_options[index].value == value->as.enum_value) {
          return true;
        }
      }
      return false;
    default:
      return false;
  }
}

/* ------------------------------------------------------------------ */
/* Manifest 流式编码：同一遍历器服务于 CRC、总长与分片窗口提取 */

typedef void (*dctp_emit_fn)(void *ctx, const uint8_t *bytes, size_t len);

static void emit_u8(dctp_emit_fn emit, void *ctx, uint8_t value) {
  emit(ctx, &value, 1);
}

static void emit_u16(dctp_emit_fn emit, void *ctx, uint16_t value) {
  uint8_t bytes[2] = {(uint8_t)(value & 0xFFu), (uint8_t)(value >> 8)};
  emit(ctx, bytes, sizeof bytes);
}

static void emit_u32(dctp_emit_fn emit, void *ctx, uint32_t value) {
  uint8_t bytes[4] = {(uint8_t)(value & 0xFFu), (uint8_t)((value >> 8) & 0xFFu),
                      (uint8_t)((value >> 16) & 0xFFu), (uint8_t)(value >> 24)};
  emit(ctx, bytes, sizeof bytes);
}

static void emit_str(dctp_emit_fn emit, void *ctx, const char *value) {
  size_t len = value != NULL ? strlen(value) : 0;
  emit_u8(emit, ctx, (uint8_t)len);
  if (len > 0) {
    emit(ctx, (const uint8_t *)value, len);
  }
}

static void emit_tagged_value(dctp_emit_fn emit, void *ctx, const dctp_value_t *value) {
  uint8_t scratch[5];
  dctp_writer_t writer;
  dctp_writer_init(&writer, scratch, sizeof scratch);
  put_tagged_value(&writer, value);
  emit(ctx, scratch, writer.len);
}

static size_t str_field_len(const char *value) {
  return 1u + (value != NULL ? strlen(value) : 0);
}

static size_t param_record_len(const dctp_param_descriptor_t *descriptor) {
  size_t len = 6u; /* param_id + type + flags */
  len += str_field_len(descriptor->machine_name);
  len += str_field_len(descriptor->display_name);
  len += str_field_len(descriptor->group);
  len += str_field_len(descriptor->unit);
  len += tagged_value_len(descriptor->default_value.type);
  len += 1u; /* constraint kind */
  if (descriptor->constraint_kind == DCTP_CONSTRAINT_NUMERIC) {
    len += 3u * tagged_value_len(descriptor->type);
  } else if (descriptor->constraint_kind == DCTP_CONSTRAINT_ENUM) {
    len += 1u;
    for (uint8_t index = 0; index < descriptor->enum_option_count; index += 1) {
      len += 4u + str_field_len(descriptor->enum_options[index].label);
    }
  }
  return len;
}

static void emit_param_record(dctp_emit_fn emit, void *ctx, const dctp_param_descriptor_t *descriptor) {
  emit_u32(emit, ctx, descriptor->param_id);
  emit_u8(emit, ctx, descriptor->type);
  emit_u8(emit, ctx, descriptor->flags);
  emit_str(emit, ctx, descriptor->machine_name);
  emit_str(emit, ctx, descriptor->display_name);
  emit_str(emit, ctx, descriptor->group);
  emit_str(emit, ctx, descriptor->unit);
  emit_tagged_value(emit, ctx, &descriptor->default_value);
  if (descriptor->constraint_kind == DCTP_CONSTRAINT_NUMERIC) {
    emit_u8(emit, ctx, 1u);
    emit_tagged_value(emit, ctx, &descriptor->min);
    emit_tagged_value(emit, ctx, &descriptor->max);
    emit_tagged_value(emit, ctx, &descriptor->step);
  } else if (descriptor->constraint_kind == DCTP_CONSTRAINT_ENUM) {
    emit_u8(emit, ctx, 2u);
    emit_u8(emit, ctx, descriptor->enum_option_count);
    for (uint8_t index = 0; index < descriptor->enum_option_count; index += 1) {
      emit_u32(emit, ctx, (uint32_t)descriptor->enum_options[index].value);
      emit_str(emit, ctx, descriptor->enum_options[index].label);
    }
  } else {
    emit_u8(emit, ctx, 0u);
  }
}

static size_t channel_record_len(const dctp_channel_descriptor_t *descriptor) {
  return 5u + str_field_len(descriptor->machine_name) + str_field_len(descriptor->display_name) +
         str_field_len(descriptor->group) + str_field_len(descriptor->unit);
}

static void emit_channel_record(dctp_emit_fn emit, void *ctx, const dctp_channel_descriptor_t *descriptor) {
  emit_u32(emit, ctx, descriptor->channel_id);
  emit_u8(emit, ctx, descriptor->type);
  emit_str(emit, ctx, descriptor->machine_name);
  emit_str(emit, ctx, descriptor->display_name);
  emit_str(emit, ctx, descriptor->group);
  emit_str(emit, ctx, descriptor->unit);
}

static void manifest_walk(const dctp_device_t *device, dctp_emit_fn emit, void *ctx) {
  emit_u16(emit, ctx, DCTP_MANIFEST_SCHEMA_VERSION);
  emit_u16(emit, ctx, device->config.param_count);
  emit_u16(emit, ctx, device->config.channel_count);
  for (uint16_t index = 0; index < device->config.param_count; index += 1) {
    const dctp_param_descriptor_t *descriptor = &device->config.params[index];
    emit_u16(emit, ctx, (uint16_t)param_record_len(descriptor));
    emit_param_record(emit, ctx, descriptor);
  }
  for (uint16_t index = 0; index < device->config.channel_count; index += 1) {
    const dctp_channel_descriptor_t *descriptor = &device->config.channels[index];
    emit_u16(emit, ctx, (uint16_t)channel_record_len(descriptor));
    emit_channel_record(emit, ctx, descriptor);
  }
}

typedef struct {
  uint32_t state;
} crc_emit_ctx_t;

static void crc_emit(void *ctx, const uint8_t *bytes, size_t len) {
  crc_emit_ctx_t *crc = (crc_emit_ctx_t *)ctx;
  crc->state = dctp_crc32_update(crc->state, bytes, len);
}

typedef struct {
  uint32_t total;
} count_emit_ctx_t;

static void count_emit(void *ctx, const uint8_t *bytes, size_t len) {
  (void)bytes;
  count_emit_ctx_t *count = (count_emit_ctx_t *)ctx;
  count->total += (uint32_t)len;
}

typedef struct {
  uint8_t *out;
  uint32_t start;
  uint32_t end;
  uint32_t position;
} window_emit_ctx_t;

static void window_emit(void *ctx, const uint8_t *bytes, size_t len) {
  window_emit_ctx_t *window = (window_emit_ctx_t *)ctx;
  uint32_t chunk_start = window->position;
  uint32_t chunk_end = chunk_start + (uint32_t)len;
  window->position = chunk_end;
  if (chunk_end <= window->start || chunk_start >= window->end) {
    return;
  }
  uint32_t copy_from = chunk_start < window->start ? window->start : chunk_start;
  uint32_t copy_to = chunk_end > window->end ? window->end : chunk_end;
  memcpy(window->out + (copy_from - window->start), bytes + (copy_from - chunk_start), copy_to - copy_from);
}

/* ------------------------------------------------------------------ */
/* 发送 */

static uint8_t *payload_buf(dctp_device_t *device) {
  return device->tx_raw + DCTP_HEADER_LEN;
}

static void send_frame(dctp_device_t *device, uint8_t message_type, uint8_t flags, uint16_t sequence,
                       uint32_t session_id, uint16_t payload_len) {
  size_t total = dctp_frame_finalize(device->tx_raw, message_type, flags, sequence, session_id, payload_len);
  if (total > 0) {
    dctp_cobs_send(device->tx_raw, total, device->config.write, device->config.user);
  }
}

/* ------------------------------------------------------------------ */
/* 可靠请求幂等缓存 */

static const dctp_cache_entry_t *cache_get(const dctp_device_t *device, uint32_t session_id, uint8_t request_type,
                                           uint16_t sequence) {
  for (uint8_t index = 0; index < device->cache_len; index += 1) {
    const dctp_cache_entry_t *entry = &device->cache[index];
    if (entry->session_id == session_id && entry->request_type == request_type && entry->sequence == sequence) {
      return entry;
    }
  }
  return NULL;
}

static void cache_put(dctp_device_t *device, const dctp_request_t *request, uint8_t response_type,
                      uint8_t response_flags, const uint8_t *payload, uint16_t payload_len) {
  if (payload_len > DCTP_CACHE_PAYLOAD_MAX) {
    return;
  }
  dctp_cache_entry_t *entry = &device->cache[device->cache_next];
  device->cache_next = (uint8_t)((device->cache_next + 1u) % DCTP_REQUEST_CACHE_ENTRIES);
  if (device->cache_len < DCTP_REQUEST_CACHE_ENTRIES) {
    device->cache_len += 1;
  }
  entry->session_id = request->session_id;
  entry->request_type = request->type;
  entry->sequence = request->sequence;
  entry->response_type = response_type;
  entry->response_flags = response_flags;
  entry->payload_len = (uint8_t)payload_len;
  memcpy(entry->payload, payload, payload_len);
}

/* respond/respond_error 发出 dispatch 产生的单帧响应并按 sim 规则缓存。 */
static void respond(dctp_device_t *device, const dctp_request_t *request, uint8_t response_type,
                    uint8_t extra_flags, uint16_t payload_len) {
  uint8_t flags = (uint8_t)(DCTP_FLAG_RESPONSE | extra_flags);
  if (request->reliable && request->type != DCTP_MSG_SESSION_CLOSE) {
    cache_put(device, request, response_type, flags, payload_buf(device), payload_len);
  }
  send_frame(device, response_type, flags, request->sequence, request->session_id, payload_len);
}

static void build_error_payload(dctp_writer_t *writer, const dctp_request_t *request, uint16_t error_code,
                                const uint8_t *context, size_t context_len) {
  dctp_put_u8(writer, request->type);
  dctp_put_u16(writer, request->sequence);
  dctp_put_u16(writer, error_code);
  dctp_put_u8(writer, (uint8_t)context_len);
  dctp_put_bytes(writer, context, context_len);
}

static void respond_error_context(dctp_device_t *device, const dctp_request_t *request, uint16_t error_code,
                                  const uint8_t *context, size_t context_len) {
  if (context_len > DCTP_MAX_ERROR_CONTEXT_LEN) {
    context_len = 0;
  }
  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  build_error_payload(&writer, request, error_code, context, context_len);
  respond(device, request, DCTP_MSG_ERROR, DCTP_FLAG_ERROR, (uint16_t)writer.len);
}

static void respond_error(dctp_device_t *device, const dctp_request_t *request, uint16_t error_code) {
  respond_error_context(device, request, error_code, NULL, 0);
}

/* 会话/长度前置校验产生的错误不进入幂等缓存（对齐 sim 的处理顺序）。 */
static void send_error_uncached(dctp_device_t *device, const dctp_request_t *request, uint16_t error_code) {
  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  build_error_payload(&writer, request, error_code, NULL, 0);
  send_frame(device, DCTP_MSG_ERROR, DCTP_FLAG_RESPONSE | DCTP_FLAG_ERROR, request->sequence, request->session_id,
             (uint16_t)writer.len);
}

/* ------------------------------------------------------------------ */
/* 会话 */

static void clear_telemetry(dctp_device_t *device) {
  device->telemetry_active = false;
  device->telemetry_pending_start = false;
  device->next_telemetry_sequence = 0;
  device->pending_dropped_samples = 0;
}

static void clear_session(dctp_device_t *device) {
  device->session_active = false;
  device->cache_len = 0;
  device->cache_next = 0;
  device->hello_completed = false;
  clear_telemetry(device);
}

static void expire_session(dctp_device_t *device, uint32_t now_ms) {
  if (device->session_active && now_ms - device->session_last_valid_ms >= DCTP_SESSION_EXPIRATION_MS) {
    clear_session(device);
    device->close_completed = false;
  }
}

static uint32_t rotl32(uint32_t value, unsigned shift) {
  return (value << shift) | (value >> (32u - shift));
}

static uint32_t open_session(dctp_device_t *device, uint32_t client_nonce, uint32_t now_ms, uint16_t max_payload) {
  bool had_previous = device->session_active;
  uint32_t previous = device->session_id;
  uint32_t candidate;
  do {
    device->session_counter += 1;
    candidate = rotl32(client_nonce, 13) ^ rotl32(device->config.boot_count, 7) ^
                (device->session_counter * 0x9e3779b9u);
  } while (candidate == 0 || (had_previous && candidate == previous));
  device->session_active = true;
  device->session_id = candidate;
  device->session_last_valid_ms = now_ms;
  device->session_max_payload = max_payload;
  device->cache_len = 0;
  device->cache_next = 0;
  device->hello_completed = false;
  device->close_completed = false;
  clear_telemetry(device);
  return candidate;
}

/* ------------------------------------------------------------------ */
/* HELLO 与 SESSION_CLOSE */

static bool replay_completed_hello(dctp_device_t *device, const dctp_request_t *request, uint32_t now_ms) {
  if (!device->hello_completed || !request->reliable || !device->session_active) {
    return false;
  }
  if (request->sequence != device->hello_sequence || request->flags != device->hello_flags ||
      request->payload_len != DCTP_HELLO_PAYLOAD_LEN ||
      memcmp(request->payload, device->hello_request, DCTP_HELLO_PAYLOAD_LEN) != 0) {
    return false;
  }
  device->session_last_valid_ms = now_ms;
  memcpy(payload_buf(device), device->hello_response, DCTP_HELLO_ACK_PAYLOAD_LEN);
  send_frame(device, DCTP_MSG_HELLO_ACK, DCTP_FLAG_RESPONSE, request->sequence, device->session_id,
             DCTP_HELLO_ACK_PAYLOAD_LEN);
  return true;
}

static void handle_hello(dctp_device_t *device, const dctp_request_t *request, uint32_t now_ms) {
  if (request->session_id != 0) {
    send_error_uncached(device, request, DCTP_ERR_INVALID_SESSION);
    return;
  }
  dctp_reader_t reader;
  dctp_reader_init(&reader, request->payload, request->payload_len);
  uint32_t client_nonce = dctp_read_u32(&reader);
  uint8_t min_version = dctp_read_u8(&reader);
  uint8_t max_version = dctp_read_u8(&reader);
  uint16_t client_max_payload = dctp_read_u16(&reader);
  if (!dctp_reader_done(&reader)) {
    send_error_uncached(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  if (min_version > DCTP_PROTOCOL_VERSION || max_version < DCTP_PROTOCOL_VERSION) {
    send_error_uncached(device, request, DCTP_ERR_UNSUPPORTED_VERSION);
    return;
  }
  if (client_max_payload < DCTP_HELLO_ACK_PAYLOAD_LEN) {
    send_error_uncached(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  uint16_t negotiated = client_max_payload < DCTP_MAX_PAYLOAD ? client_max_payload : DCTP_MAX_PAYLOAD;
  uint32_t session_id = open_session(device, client_nonce, now_ms, negotiated);

  uint32_t capabilities = DCTP_CAP_PARAMETERS | DCTP_CAP_TELEMETRY | DCTP_CAP_STRUCTURED_LOG;
  if (device->config.persist != NULL) {
    capabilities |= DCTP_CAP_PERSISTENCE;
  }
  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  dctp_put_u32(&writer, session_id);
  dctp_put_bytes(&writer, device->config.device_id, DCTP_DEVICE_ID_LEN);
  dctp_put_u32(&writer, device->config.boot_count);
  dctp_put_u16(&writer, device->config.firmware_major);
  dctp_put_u16(&writer, device->config.firmware_minor);
  dctp_put_u16(&writer, device->config.firmware_patch);
  dctp_put_u16(&writer, DCTP_SDK_VERSION_MAJOR);
  dctp_put_u16(&writer, DCTP_SDK_VERSION_MINOR);
  dctp_put_u16(&writer, DCTP_SDK_VERSION_PATCH);
  dctp_put_u32(&writer, capabilities);
  dctp_put_u32(&writer, device->manifest_crc32);
  dctp_put_u16(&writer, negotiated);

  if (request->reliable) {
    device->hello_completed = true;
    device->hello_sequence = request->sequence;
    device->hello_flags = request->flags;
    memcpy(device->hello_request, request->payload, DCTP_HELLO_PAYLOAD_LEN);
    memcpy(device->hello_response, payload_buf(device), DCTP_HELLO_ACK_PAYLOAD_LEN);
  }
  send_frame(device, DCTP_MSG_HELLO_ACK, DCTP_FLAG_RESPONSE, request->sequence, session_id,
             DCTP_HELLO_ACK_PAYLOAD_LEN);
}

static bool replay_completed_close(dctp_device_t *device, const dctp_request_t *request) {
  if (!device->close_completed || !request->reliable) {
    return false;
  }
  if (request->sequence != device->close_sequence || request->flags != device->close_flags ||
      request->payload_len != 0 || request->session_id != device->close_session_id) {
    return false;
  }
  send_frame(device, DCTP_MSG_SESSION_CLOSE, DCTP_FLAG_RESPONSE, request->sequence, device->close_session_id, 0);
  return true;
}

static void handle_session_close(dctp_device_t *device, const dctp_request_t *request) {
  if (request->payload_len != 0) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  bool remember = request->reliable;
  uint16_t sequence = request->sequence;
  uint8_t flags = request->flags;
  uint32_t session_id = request->session_id;
  clear_session(device);
  device->close_completed = remember;
  device->close_sequence = sequence;
  device->close_flags = flags;
  device->close_session_id = session_id;
  send_frame(device, DCTP_MSG_SESSION_CLOSE, DCTP_FLAG_RESPONSE, sequence, session_id, 0);
}

/* ------------------------------------------------------------------ */
/* 会话内请求 */

static void handle_heartbeat(dctp_device_t *device, const dctp_request_t *request) {
  if (request->payload_len != 4u) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  memcpy(payload_buf(device), request->payload, 4u);
  respond(device, request, DCTP_MSG_HEARTBEAT_ACK, 0, 4u);
}

static void handle_manifest_request(dctp_device_t *device, const dctp_request_t *request) {
  if (request->payload_len != 0) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  uint32_t total_len = device->manifest_total_len;
  uint32_t chunk_data_len = (uint32_t)device->session_max_payload - DCTP_MANIFEST_CHUNK_PREFIX_LEN;
  for (uint32_t offset = 0; offset < total_len; offset += chunk_data_len) {
    uint32_t data_len = total_len - offset < chunk_data_len ? total_len - offset : chunk_data_len;
    dctp_writer_t writer;
    dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
    dctp_put_u32(&writer, device->manifest_crc32);
    dctp_put_u32(&writer, total_len);
    dctp_put_u32(&writer, offset);
    window_emit_ctx_t window = {
        .out = payload_buf(device) + DCTP_MANIFEST_CHUNK_PREFIX_LEN,
        .start = offset,
        .end = offset + data_len,
        .position = 0,
    };
    manifest_walk(device, window_emit, &window);
    send_frame(device, DCTP_MSG_MANIFEST_CHUNK, DCTP_FLAG_RESPONSE | DCTP_FLAG_MORE_FRAGMENTS, request->sequence,
               request->session_id, (uint16_t)(DCTP_MANIFEST_CHUNK_PREFIX_LEN + data_len));
  }
  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  dctp_put_u32(&writer, device->manifest_crc32);
  dctp_put_u32(&writer, total_len);
  send_frame(device, DCTP_MSG_MANIFEST_DONE, DCTP_FLAG_RESPONSE, request->sequence, request->session_id,
             (uint16_t)writer.len);
}

static uint16_t build_param_state_payload(dctp_device_t *device, int param_index) {
  const dctp_param_descriptor_t *descriptor = &device->config.params[param_index];
  const dctp_param_state_t *state = &device->param_state[param_index];
  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  dctp_put_u32(&writer, descriptor->param_id);
  dctp_put_u32(&writer, state->revision);
  put_tagged_value(&writer, &state->value);
  if (state->has_persisted) {
    dctp_put_u8(&writer, 1u);
    put_tagged_value(&writer, &state->persisted_value);
  } else {
    dctp_put_u8(&writer, 0u);
  }
  return (uint16_t)writer.len;
}

static void handle_param_read(dctp_device_t *device, const dctp_request_t *request) {
  if (request->payload_len != 4u) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  dctp_reader_t reader;
  dctp_reader_init(&reader, request->payload, request->payload_len);
  uint32_t param_id = dctp_read_u32(&reader);
  int param_index = find_param(device, param_id);
  if (param_index < 0) {
    respond_error(device, request, DCTP_ERR_INVALID_PARAM_ID);
    return;
  }
  respond(device, request, DCTP_MSG_PARAM_VALUE, 0, build_param_state_payload(device, param_index));
}

static uint16_t build_write_ack_bytes(uint8_t *out, size_t capacity, const dctp_value_t *value, uint32_t revision) {
  dctp_writer_t writer;
  dctp_writer_init(&writer, out, capacity);
  put_tagged_value(&writer, value);
  dctp_put_u32(&writer, revision);
  return (uint16_t)writer.len;
}

static void handle_param_write(dctp_device_t *device, const dctp_request_t *request) {
  dctp_reader_t reader;
  dctp_reader_init(&reader, request->payload, request->payload_len);
  uint32_t param_id = dctp_read_u32(&reader);
  uint32_t expected_revision = dctp_read_u32(&reader);
  dctp_value_t value;
  if (!read_tagged_value(&reader, &value) || !dctp_reader_done(&reader)) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  int param_index = find_param(device, param_id);
  if (param_index < 0) {
    respond_error(device, request, DCTP_ERR_INVALID_PARAM_ID);
    return;
  }
  const dctp_param_descriptor_t *descriptor = &device->config.params[param_index];
  dctp_param_state_t *state = &device->param_state[param_index];
  if (descriptor->type != value.type) {
    respond_error(device, request, DCTP_ERR_TYPE_MISMATCH);
    return;
  }
  if ((descriptor->flags & DCTP_PARAM_WRITABLE) == 0) {
    respond_error(device, request, DCTP_ERR_READ_ONLY);
    return;
  }
  if (!value_within_constraints(&value, descriptor)) {
    respond_error(device, request, DCTP_ERR_OUT_OF_RANGE);
    return;
  }
  if (state->revision != expected_revision) {
    /* 冲突上下文携带当前值 ACK 的小写十六进制字节，客户端可无损恢复。 */
    uint8_t ack_bytes[9];
    uint16_t ack_len = build_write_ack_bytes(ack_bytes, sizeof ack_bytes, &state->value, state->revision);
    uint8_t context[18];
    static const char digits[] = "0123456789abcdef";
    for (uint16_t index = 0; index < ack_len; index += 1) {
      context[index * 2u] = (uint8_t)digits[ack_bytes[index] >> 4];
      context[index * 2u + 1u] = (uint8_t)digits[ack_bytes[index] & 0x0Fu];
    }
    respond_error_context(device, request, DCTP_ERR_REVISION_CONFLICT, context, (size_t)ack_len * 2u);
    return;
  }

  state->value = value;
  state->revision += 1;
  uint16_t payload_len = build_write_ack_bytes(payload_buf(device), DCTP_MAX_PAYLOAD, &state->value, state->revision);
  respond(device, request, DCTP_MSG_PARAM_WRITE_ACK, 0, payload_len);
}

static uint32_t commit_entry_id(const uint8_t *payload, uint16_t index) {
  const uint8_t *entry = payload + 2u + (size_t)index * 8u;
  return (uint32_t)entry[0] | ((uint32_t)entry[1] << 8) | ((uint32_t)entry[2] << 16) | ((uint32_t)entry[3] << 24);
}

static bool commit_contains(const uint8_t *payload, uint16_t entry_count, uint32_t param_id) {
  for (uint16_t index = 0; index < entry_count; index += 1) {
    if (commit_entry_id(payload, index) == param_id) {
      return true;
    }
  }
  return false;
}

/*
 * 生成 A/B 槽记录：提交条目取当前 RAM 值，其余持久化参数保留既有影子值。
 * 只有 persist 成功后才把这一集合落到设备状态。
 */
static uint32_t build_commit_blob(dctp_device_t *device, uint32_t generation, const uint8_t *commit_payload,
                                  uint16_t entry_count) {
  dctp_writer_t writer;
  dctp_writer_init(&writer, device->storage_blob, sizeof device->storage_blob);
  dctp_put_u32(&writer, DCTP_STORAGE_MAGIC);
  dctp_put_u16(&writer, DCTP_STORAGE_VERSION);
  dctp_put_u16(&writer, 0u);
  dctp_put_u32(&writer, device->manifest_crc32);
  dctp_put_u32(&writer, generation);
  size_t payload_len_offset = writer.len;
  dctp_put_u32(&writer, 0u);
  size_t payload_start = writer.len;
  for (uint16_t index = 0; index < device->config.param_count; index += 1) {
    const dctp_param_state_t *state = &device->param_state[index];
    uint32_t param_id = device->config.params[index].param_id;
    if ((device->config.params[index].flags & DCTP_PARAM_PERSISTENT) == 0) {
      continue;
    }
    const dctp_value_t *value;
    if (commit_contains(commit_payload, entry_count, param_id)) {
      value = &state->value;
    } else if (state->has_persisted) {
      value = &state->persisted_value;
    } else {
      continue;
    }
    dctp_put_u32(&writer, param_id);
    put_tagged_value(&writer, value);
  }
  if (!writer.ok) {
    return 0;
  }
  uint32_t payload_len = (uint32_t)(writer.len - payload_start);
  device->storage_blob[payload_len_offset] = (uint8_t)(payload_len & 0xFFu);
  device->storage_blob[payload_len_offset + 1] = (uint8_t)((payload_len >> 8) & 0xFFu);
  device->storage_blob[payload_len_offset + 2] = (uint8_t)((payload_len >> 16) & 0xFFu);
  device->storage_blob[payload_len_offset + 3] = (uint8_t)(payload_len >> 24);
  uint32_t crc = dctp_crc32(device->storage_blob, writer.len);
  dctp_put_u32(&writer, crc);
  return writer.ok ? (uint32_t)writer.len : 0;
}

static void handle_param_commit(dctp_device_t *device, const dctp_request_t *request) {
  dctp_reader_t reader;
  dctp_reader_init(&reader, request->payload, request->payload_len);
  uint16_t entry_count = dctp_read_u16(&reader);
  size_t expected_len = 2u + (size_t)entry_count * 8u + 4u;
  if (!reader.ok || request->payload_len != expected_len) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }

  /* 第一遍：校验条目并计算 canonical CRC32（条目已按协议要求严格升序）。 */
  uint32_t crc_state = DCTP_CRC32_INIT;
  uint32_t previous_id = 0;
  for (uint16_t index = 0; index < entry_count; index += 1) {
    uint32_t param_id = dctp_read_u32(&reader);
    uint32_t revision = dctp_read_u32(&reader);
    if (index > 0 && param_id <= previous_id) {
      respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
      return;
    }
    previous_id = param_id;
    int param_index = find_param(device, param_id);
    if (param_index < 0) {
      respond_error(device, request, DCTP_ERR_INVALID_PARAM_ID);
      return;
    }
    if ((device->config.params[param_index].flags & DCTP_PARAM_PERSISTENT) == 0) {
      respond_error(device, request, DCTP_ERR_READ_ONLY);
      return;
    }
    if (device->param_state[param_index].revision != revision) {
      respond_error(device, request, DCTP_ERR_REVISION_CONFLICT);
      return;
    }
    crc_state = canonical_crc_update(crc_state, param_id, &device->param_state[param_index].value);
  }
  uint32_t canonical_crc32 = dctp_crc32_finish(crc_state);
  uint32_t requested_crc32 = dctp_read_u32(&reader);
  if (requested_crc32 != canonical_crc32) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  if (device->config.persist == NULL) {
    respond_error(device, request, DCTP_ERR_STORAGE_FAILED);
    return;
  }

  /* 先写非活动槽并读回校验；设备状态只有在成功后才改变。 */
  uint32_t next_generation = device->storage_generation + 1;
  uint32_t blob_len = build_commit_blob(device, next_generation, request->payload, entry_count);
  int persist_result = blob_len > 0 ? device->config.persist(device->config.user, device->storage_blob, blob_len)
                                    : DCTP_PERSIST_STORAGE_FAILED;
  if (persist_result != DCTP_PERSIST_OK) {
    respond_error(device, request,
                  persist_result == DCTP_PERSIST_VERIFY_FAILED ? DCTP_ERR_VERIFY_FAILED : DCTP_ERR_STORAGE_FAILED);
    return;
  }
  for (uint16_t index = 0; index < entry_count; index += 1) {
    int param_index = find_param(device, commit_entry_id(request->payload, index));
    dctp_param_state_t *state = &device->param_state[param_index];
    state->persisted_value = state->value;
    state->has_persisted = 1;
  }
  device->storage_generation = next_generation;

  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  dctp_put_u32(&writer, canonical_crc32);
  dctp_put_u32(&writer, device->storage_generation);
  respond(device, request, DCTP_MSG_PARAM_COMMIT_ACK, 0, (uint16_t)writer.len);
}

static void handle_telemetry_subscribe(dctp_device_t *device, const dctp_request_t *request) {
  dctp_reader_t reader;
  dctp_reader_init(&reader, request->payload, request->payload_len);
  uint16_t subscription_version = dctp_read_u16(&reader);
  uint16_t sample_rate_hz = dctp_read_u16(&reader);
  uint8_t channel_count = dctp_read_u8(&reader);
  if (!reader.ok || channel_count == 0 || channel_count > DCTP_TELEMETRY_MAX_SUBSCRIBED ||
      request->payload_len != 5u + (size_t)channel_count * 4u || sample_rate_hz == 0 || sample_rate_hz > 500u) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  uint8_t channel_index[DCTP_TELEMETRY_MAX_SUBSCRIBED];
  uint32_t channel_ids[DCTP_TELEMETRY_MAX_SUBSCRIBED];
  for (uint8_t index = 0; index < channel_count; index += 1) {
    uint32_t channel_id = dctp_read_u32(&reader);
    for (uint8_t previous = 0; previous < index; previous += 1) {
      if (channel_ids[previous] == channel_id) {
        respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
        return;
      }
    }
    channel_ids[index] = channel_id;
    int found = find_channel(device, channel_id);
    if (found < 0) {
      respond_error(device, request, DCTP_ERR_INVALID_PARAM_ID);
      return;
    }
    channel_index[index] = (uint8_t)found;
  }

  device->subscription.subscription_version = subscription_version;
  device->subscription.sample_rate_hz = sample_rate_hz;
  device->subscription.channel_count = channel_count;
  memcpy(device->subscription.channel_index, channel_index, channel_count);
  device->telemetry_active = true;
  device->telemetry_pending_start = true;
  device->next_telemetry_sequence = 0;
  device->pending_dropped_samples = 0;
  respond(device, request, DCTP_MSG_TELEMETRY_SUBSCRIBE_ACK, 0, 0);
}

static void handle_telemetry_stop(dctp_device_t *device, const dctp_request_t *request) {
  if (request->payload_len != 0) {
    respond_error(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  clear_telemetry(device);
  respond(device, request, DCTP_MSG_TELEMETRY_STOP, 0, 0);
}

/* ------------------------------------------------------------------ */
/* 帧调度 */

static bool is_known_message_type(uint8_t value) {
  switch (value) {
    case DCTP_MSG_HELLO:
    case DCTP_MSG_HELLO_ACK:
    case DCTP_MSG_HEARTBEAT:
    case DCTP_MSG_HEARTBEAT_ACK:
    case DCTP_MSG_SESSION_CLOSE:
    case DCTP_MSG_MANIFEST_REQUEST:
    case DCTP_MSG_MANIFEST_CHUNK:
    case DCTP_MSG_MANIFEST_DONE:
    case DCTP_MSG_PARAM_READ:
    case DCTP_MSG_PARAM_VALUE:
    case DCTP_MSG_PARAM_WRITE:
    case DCTP_MSG_PARAM_WRITE_ACK:
    case DCTP_MSG_PARAM_COMMIT:
    case DCTP_MSG_PARAM_COMMIT_ACK:
    case DCTP_MSG_TELEMETRY_SUBSCRIBE:
    case DCTP_MSG_TELEMETRY_SUBSCRIBE_ACK:
    case DCTP_MSG_TELEMETRY_DATA:
    case DCTP_MSG_TELEMETRY_STOP:
    case DCTP_MSG_LOG_MESSAGE:
    case DCTP_MSG_DEVICE_EVENT:
    case DCTP_MSG_PREPARE_FLASH:
    case DCTP_MSG_PREPARE_FLASH_ACK:
    case DCTP_MSG_ERROR:
      return true;
    default:
      return false;
  }
}

static void handle_request(dctp_device_t *device, const dctp_request_t *request, uint32_t now_ms) {
  expire_session(device, now_ms);

  if (request->type == DCTP_MSG_HELLO) {
    if (!replay_completed_hello(device, request, now_ms)) {
      handle_hello(device, request, now_ms);
    }
    return;
  }
  if (request->type == DCTP_MSG_SESSION_CLOSE && replay_completed_close(device, request)) {
    return;
  }
  if (!device->session_active || request->session_id != device->session_id) {
    send_error_uncached(device, request, DCTP_ERR_INVALID_SESSION);
    return;
  }
  if (request->payload_len > device->session_max_payload) {
    send_error_uncached(device, request, DCTP_ERR_INVALID_LENGTH);
    return;
  }
  device->session_last_valid_ms = now_ms;

  if (request->reliable) {
    const dctp_cache_entry_t *cached = cache_get(device, request->session_id, request->type, request->sequence);
    if (cached != NULL) {
      memcpy(payload_buf(device), cached->payload, cached->payload_len);
      send_frame(device, cached->response_type, cached->response_flags, cached->sequence, cached->session_id,
                 cached->payload_len);
      return;
    }
  }

  switch (request->type) {
    case DCTP_MSG_HEARTBEAT:
      handle_heartbeat(device, request);
      break;
    case DCTP_MSG_MANIFEST_REQUEST:
      handle_manifest_request(device, request);
      break;
    case DCTP_MSG_PARAM_READ:
      handle_param_read(device, request);
      break;
    case DCTP_MSG_PARAM_WRITE:
      handle_param_write(device, request);
      break;
    case DCTP_MSG_PARAM_COMMIT:
      handle_param_commit(device, request);
      break;
    case DCTP_MSG_TELEMETRY_SUBSCRIBE:
      handle_telemetry_subscribe(device, request);
      break;
    case DCTP_MSG_TELEMETRY_STOP:
      handle_telemetry_stop(device, request);
      break;
    case DCTP_MSG_SESSION_CLOSE:
      handle_session_close(device, request);
      break;
    default:
      respond_error(device, request, DCTP_ERR_UNKNOWN_MESSAGE);
      break;
  }
}

/* rx_frame 持有已通过 CRC 的完整解码帧。 */
static void process_frame(dctp_device_t *device, uint32_t now_ms) {
  const uint8_t *raw = device->rx_frame;
  uint16_t total_len = device->rx_len;
  if (total_len < DCTP_HEADER_LEN + 2u) {
    device->rx_malformed_frames += 1;
    return;
  }
  uint16_t magic = (uint16_t)(raw[0] | ((uint16_t)raw[1] << 8));
  if (magic != DCTP_MAGIC || raw[2] != DCTP_PROTOCOL_VERSION) {
    device->rx_malformed_frames += 1;
    return;
  }
  uint16_t payload_len = (uint16_t)(raw[11] | ((uint16_t)raw[12] << 8));
  if (payload_len > DCTP_MAX_PAYLOAD || total_len != DCTP_HEADER_LEN + payload_len + 2u) {
    device->rx_malformed_frames += 1;
    return;
  }
  uint16_t expected_crc = (uint16_t)(raw[total_len - 2u] | ((uint16_t)raw[total_len - 1u] << 8));
  if (dctp_crc16_ccitt_false(raw, (size_t)total_len - 2u) != expected_crc) {
    device->rx_malformed_frames += 1;
    return;
  }
  if (!is_known_message_type(raw[3])) {
    device->rx_malformed_frames += 1;
    return;
  }

  dctp_request_t request;
  request.type = raw[3];
  request.flags = raw[4];
  request.sequence = (uint16_t)(raw[5] | ((uint16_t)raw[6] << 8));
  request.session_id = (uint32_t)raw[7] | ((uint32_t)raw[8] << 8) | ((uint32_t)raw[9] << 16) |
                       ((uint32_t)raw[10] << 24);
  request.payload = raw + DCTP_HEADER_LEN;
  request.payload_len = payload_len;
  request.reliable = (request.flags & DCTP_FLAG_ACK_REQUIRED) != 0;
  handle_request(device, &request, now_ms);
}

/* ------------------------------------------------------------------ */
/* 公共 API */

static void reset_rx(dctp_device_t *device) {
  device->rx_len = 0;
  device->rx_block_remaining = 0;
  device->rx_block_append_zero = false;
  device->rx_dropping = false;
  device->rx_started = false;
}

void dctp_device_rx(dctp_device_t *device, const uint8_t *bytes, size_t len, uint32_t now_ms) {
  for (size_t index = 0; index < len; index += 1) {
    uint8_t byte = bytes[index];

    if (device->rx_dropping) {
      if (byte == 0) {
        device->rx_malformed_frames += 1;
        reset_rx(device);
      }
      continue;
    }

    if (byte == 0) {
      if (!device->rx_started) {
        continue;
      }
      if (device->rx_block_remaining != 0) {
        device->rx_malformed_frames += 1;
      } else {
        process_frame(device, now_ms);
      }
      reset_rx(device);
      continue;
    }

    if (device->rx_block_remaining > 0) {
      if (device->rx_len >= DCTP_RAW_FRAME_MAX) {
        device->rx_dropping = true;
        continue;
      }
      device->rx_frame[device->rx_len++] = byte;
      device->rx_block_remaining -= 1;
      continue;
    }

    /* 块之间：如上一块非 0xFF 需补回一个 0x00，再把当前字节作为新 code。 */
    if (device->rx_block_append_zero) {
      if (device->rx_len >= DCTP_RAW_FRAME_MAX) {
        device->rx_dropping = true;
        continue;
      }
      device->rx_frame[device->rx_len++] = 0;
    }
    device->rx_started = true;
    device->rx_block_remaining = (uint8_t)(byte - 1u);
    device->rx_block_append_zero = byte != 0xFFu;
  }
}

void dctp_device_poll(dctp_device_t *device, uint32_t now_ms, uint32_t now_us) {
  expire_session(device, now_ms);
  if (!device->session_active || !device->telemetry_active) {
    return;
  }

  uint32_t period_us = 1000000u / device->subscription.sample_rate_hz;
  if (device->telemetry_pending_start) {
    device->telemetry_pending_start = false;
    device->next_telemetry_at_us = now_us + period_us;
    return;
  }
  int32_t overdue_us = (int32_t)(now_us - device->next_telemetry_at_us);
  if (overdue_us < 0) {
    return;
  }

  uint8_t channel_count = device->subscription.channel_count;
  uint32_t sample_len = 2u + (uint32_t)channel_count * 4u;
  uint32_t payload_capacity =
      ((uint32_t)device->session_max_payload - DCTP_TELEMETRY_BATCH_PREFIX_LEN) / sample_len;
  uint32_t delta_capacity = period_us <= 0xFFFFu ? DCTP_TELEMETRY_MAX_SAMPLES : 1u;
  uint32_t capacity = payload_capacity < delta_capacity ? payload_capacity : delta_capacity;
  if (capacity > DCTP_TELEMETRY_MAX_SAMPLES) {
    capacity = DCTP_TELEMETRY_MAX_SAMPLES;
  }
  if (capacity == 0) {
    clear_telemetry(device);
    return;
  }

  uint32_t due = (uint32_t)overdue_us / period_us + 1u;
  uint32_t emitted = due < capacity ? due : capacity;
  uint32_t skipped = due - emitted;
  uint32_t pending = (uint32_t)device->pending_dropped_samples + skipped;
  device->pending_dropped_samples = pending > 0xFFFFu ? 0xFFFFu : (uint16_t)pending;
  uint16_t first_sequence = (uint16_t)(device->next_telemetry_sequence + (uint16_t)skipped);
  device->next_telemetry_sequence = (uint16_t)(first_sequence + (uint16_t)emitted);
  uint32_t base_timestamp_us = device->next_telemetry_at_us + skipped * period_us;
  device->next_telemetry_at_us += due * period_us;

  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  dctp_put_u16(&writer, device->subscription.subscription_version);
  dctp_put_u16(&writer, first_sequence);
  dctp_put_u8(&writer, (uint8_t)emitted);
  dctp_put_u8(&writer, channel_count);
  dctp_put_u16(&writer, device->pending_dropped_samples);
  dctp_put_u32(&writer, base_timestamp_us);
  for (uint32_t sample = 0; sample < emitted; sample += 1) {
    dctp_put_u16(&writer, sample == 0 ? 0u : (uint16_t)period_us);
    for (uint8_t channel = 0; channel < channel_count; channel += 1) {
      const dctp_channel_descriptor_t *descriptor =
          &device->config.channels[device->subscription.channel_index[channel]];
      dctp_put_u32(&writer, descriptor->read(device->config.user));
    }
  }
  if (!writer.ok) {
    return;
  }

  size_t raw_len = DCTP_HEADER_LEN + writer.len + 2u;
  if (device->config.tx_free != NULL &&
      device->config.tx_free(device->config.user) < dctp_cobs_encoded_len(raw_len)) {
    uint32_t lost = (uint32_t)device->pending_dropped_samples + emitted;
    device->pending_dropped_samples = lost > 0xFFFFu ? 0xFFFFu : (uint16_t)lost;
    device->dropped_telemetry_frames += 1;
    return;
  }
  device->pending_dropped_samples = 0;
  send_frame(device, DCTP_MSG_TELEMETRY_DATA, 0, first_sequence, device->session_id, (uint16_t)writer.len);
}

bool dctp_device_log(dctp_device_t *device, uint8_t severity, uint16_t module_id, uint32_t timestamp_us,
                     const char *text) {
  if (!device->session_active || severity > 4u || text == NULL) {
    return false;
  }
  size_t text_len = strlen(text);
  if (text_len > DCTP_MAX_LOG_TEXT_LEN) {
    return false;
  }
  size_t payload_len = 8u + text_len;
  if (payload_len > device->session_max_payload) {
    return false;
  }
  size_t raw_len = DCTP_HEADER_LEN + payload_len + 2u;
  if (device->config.tx_free != NULL &&
      device->config.tx_free(device->config.user) < dctp_cobs_encoded_len(raw_len)) {
    device->dropped_log_messages += 1;
    return false;
  }
  dctp_writer_t writer;
  dctp_writer_init(&writer, payload_buf(device), DCTP_MAX_PAYLOAD);
  dctp_put_u32(&writer, timestamp_us);
  dctp_put_u8(&writer, severity);
  dctp_put_u16(&writer, module_id);
  dctp_put_u8(&writer, (uint8_t)text_len);
  dctp_put_bytes(&writer, (const uint8_t *)text, text_len);
  device->log_sequence += 1;
  send_frame(device, DCTP_MSG_LOG_MESSAGE, 0, device->log_sequence, device->session_id, (uint16_t)writer.len);
  return true;
}

bool dctp_device_get_value(const dctp_device_t *device, uint32_t param_id, dctp_value_t *out) {
  int param_index = find_param(device, param_id);
  if (param_index < 0) {
    return false;
  }
  *out = device->param_state[param_index].value;
  return true;
}

bool dctp_device_set_value(dctp_device_t *device, uint32_t param_id, dctp_value_t value) {
  int param_index = find_param(device, param_id);
  if (param_index < 0) {
    return false;
  }
  const dctp_param_descriptor_t *descriptor = &device->config.params[param_index];
  if (descriptor->type != value.type || !value_within_constraints(&value, descriptor)) {
    return false;
  }
  device->param_state[param_index].value = value;
  device->param_state[param_index].revision += 1;
  return true;
}

/* ------------------------------------------------------------------ */
/* 持久化槽解析 */

typedef struct {
  bool valid;
  uint32_t generation;
  const uint8_t *payload;
  uint32_t payload_len;
} storage_slot_t;

static storage_slot_t parse_slot(const uint8_t *bytes, uint32_t len) {
  storage_slot_t slot = {false, 0, NULL, 0};
  if (bytes == NULL || len < 24u) {
    return slot;
  }
  dctp_reader_t reader;
  dctp_reader_init(&reader, bytes, len);
  uint32_t magic = dctp_read_u32(&reader);
  uint16_t version = dctp_read_u16(&reader);
  uint16_t reserved = dctp_read_u16(&reader);
  (void)dctp_read_u32(&reader); /* manifest_crc32：允许跨固件版本按 id/类型匹配 */
  uint32_t generation = dctp_read_u32(&reader);
  uint32_t payload_len = dctp_read_u32(&reader);
  if (!reader.ok || magic != DCTP_STORAGE_MAGIC || version != DCTP_STORAGE_VERSION || reserved != 0 ||
      len != 24u + payload_len) {
    return slot;
  }
  uint32_t stored_crc = (uint32_t)bytes[len - 4u] | ((uint32_t)bytes[len - 3u] << 8) |
                        ((uint32_t)bytes[len - 2u] << 16) | ((uint32_t)bytes[len - 1u] << 24);
  if (dctp_crc32(bytes, len - 4u) != stored_crc) {
    return slot;
  }
  slot.valid = true;
  slot.generation = generation;
  slot.payload = bytes + 20u;
  slot.payload_len = payload_len;
  return slot;
}

bool dctp_storage_apply(dctp_device_t *device, const uint8_t *slot_a, uint32_t len_a, const uint8_t *slot_b,
                        uint32_t len_b) {
  storage_slot_t first = parse_slot(slot_a, len_a);
  storage_slot_t second = parse_slot(slot_b, len_b);
  storage_slot_t chosen = first;
  if (!chosen.valid || (second.valid && second.generation > chosen.generation)) {
    chosen = second;
  }
  if (!chosen.valid) {
    return false;
  }

  dctp_reader_t reader;
  dctp_reader_init(&reader, chosen.payload, chosen.payload_len);
  while (reader.ok && reader.offset < reader.len) {
    uint32_t param_id = dctp_read_u32(&reader);
    dctp_value_t value;
    if (!read_tagged_value(&reader, &value)) {
      return false;
    }
    int param_index = find_param(device, param_id);
    if (param_index < 0) {
      continue;
    }
    const dctp_param_descriptor_t *descriptor = &device->config.params[param_index];
    if (descriptor->type != value.type || (descriptor->flags & DCTP_PARAM_PERSISTENT) == 0) {
      continue;
    }
    device->param_state[param_index].value = value;
    device->param_state[param_index].persisted_value = value;
    device->param_state[param_index].has_persisted = 1;
  }
  if (!dctp_reader_done(&reader)) {
    return false;
  }
  device->storage_generation = chosen.generation;
  return true;
}

uint32_t dctp_device_storage_generation(const dctp_device_t *device) {
  return device->storage_generation;
}

bool dctp_device_session_active(const dctp_device_t *device) {
  return device->session_active;
}

uint32_t dctp_device_manifest_crc32(const dctp_device_t *device) {
  return device->manifest_crc32;
}

/* ------------------------------------------------------------------ */
/* 初始化与描述表校验 */

static bool valid_str(const char *value, size_t max_len) {
  return value != NULL && strlen(value) <= max_len;
}

static bool valid_value_type(uint8_t type) {
  return type >= DCTP_TYPE_I32 && type <= DCTP_TYPE_ENUM;
}

static bool validate_param_descriptor(const dctp_param_descriptor_t *descriptor) {
  if (!valid_value_type(descriptor->type) || descriptor->default_value.type != descriptor->type) {
    return false;
  }
  if (!valid_str(descriptor->machine_name, 48) || !valid_str(descriptor->display_name, 64) ||
      !valid_str(descriptor->group, 32) || !valid_str(descriptor->unit, 16)) {
    return false;
  }
  if (descriptor->type == DCTP_TYPE_BOOL && descriptor->default_value.as.boolean > 1u) {
    return false;
  }
  switch (descriptor->constraint_kind) {
    case DCTP_CONSTRAINT_NONE:
      return true;
    case DCTP_CONSTRAINT_NUMERIC:
      if (descriptor->type != DCTP_TYPE_I32 && descriptor->type != DCTP_TYPE_U32 &&
          descriptor->type != DCTP_TYPE_F32) {
        return false;
      }
      return descriptor->min.type == descriptor->type && descriptor->max.type == descriptor->type &&
             descriptor->step.type == descriptor->type;
    case DCTP_CONSTRAINT_ENUM: {
      if (descriptor->type != DCTP_TYPE_ENUM || descriptor->enum_options == NULL ||
          descriptor->enum_option_count == 0 || descriptor->enum_option_count > 32u) {
        return false;
      }
      for (uint8_t index = 0; index < descriptor->enum_option_count; index += 1) {
        if (!valid_str(descriptor->enum_options[index].label, 32)) {
          return false;
        }
        for (uint8_t previous = 0; previous < index; previous += 1) {
          if (descriptor->enum_options[previous].value == descriptor->enum_options[index].value) {
            return false;
          }
        }
      }
      return true;
    }
    default:
      return false;
  }
}

static bool validate_tables(const dctp_device_config_t *config) {
  if (config->write == NULL) {
    return false;
  }
  if (config->param_count > DCTP_MAX_PARAMS || config->param_count > DCTP_PROTOCOL_MAX_PARAMS ||
      (config->param_count > 0 && config->params == NULL)) {
    return false;
  }
  if (config->channel_count > DCTP_MAX_CHANNELS || config->channel_count > DCTP_PROTOCOL_MAX_CHANNELS ||
      (config->channel_count > 0 && config->channels == NULL)) {
    return false;
  }
  for (uint16_t index = 0; index < config->param_count; index += 1) {
    if (!validate_param_descriptor(&config->params[index])) {
      return false;
    }
    if (index > 0 && config->params[index].param_id <= config->params[index - 1].param_id) {
      return false;
    }
  }
  for (uint16_t index = 0; index < config->channel_count; index += 1) {
    const dctp_channel_descriptor_t *descriptor = &config->channels[index];
    if (descriptor->type < DCTP_TELEMETRY_F32 || descriptor->type > DCTP_TELEMETRY_FLAGS32 ||
        descriptor->read == NULL) {
      return false;
    }
    if (!valid_str(descriptor->machine_name, 48) || !valid_str(descriptor->display_name, 64) ||
        !valid_str(descriptor->group, 32) || !valid_str(descriptor->unit, 16)) {
      return false;
    }
    if (index > 0 && descriptor->channel_id <= config->channels[index - 1].channel_id) {
      return false;
    }
  }
  return true;
}

bool dctp_device_init(dctp_device_t *device, const dctp_device_config_t *config) {
  if (device == NULL || config == NULL || !validate_tables(config)) {
    return false;
  }
  memset(device, 0, sizeof *device);
  device->config = *config;

  for (uint16_t index = 0; index < config->param_count; index += 1) {
    const dctp_param_descriptor_t *descriptor = &config->params[index];
    dctp_param_state_t *state = &device->param_state[index];
    state->value = descriptor->default_value;
    state->revision = 0;
    if ((descriptor->flags & DCTP_PARAM_PERSISTENT) != 0) {
      state->persisted_value = descriptor->default_value;
      state->has_persisted = 1;
    } else {
      state->has_persisted = 0;
    }
  }

  count_emit_ctx_t count = {0};
  manifest_walk(device, count_emit, &count);
  if (count.total > DCTP_MANIFEST_MAX_LEN) {
    return false;
  }
  crc_emit_ctx_t crc = {DCTP_CRC32_INIT};
  manifest_walk(device, crc_emit, &crc);
  device->manifest_total_len = count.total;
  device->manifest_crc32 = dctp_crc32_finish(crc.state);
  reset_rx(device);
  return true;
}
