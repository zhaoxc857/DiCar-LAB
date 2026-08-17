#include "dctp_device.h"
#include "tmx_firmware_flash.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

enum { TMX_TEST_TX_CAPACITY = 2048, TMX_TEST_ORDER_CAPACITY = 16 };

typedef struct {
  dctp_device_t device;
  tmx_firmware_flash_t flash;
  uint8_t tx[TMX_TEST_TX_CAPACITY];
  size_t tx_len;
  uint8_t order[TMX_TEST_ORDER_CAPACITY];
  size_t order_len;
  bool tx_complete;
  uint32_t enter_calls;
} tmx_flash_test_shim_t;

static void record(tmx_flash_test_shim_t *shim, uint8_t step) {
  if (shim->order_len < TMX_TEST_ORDER_CAPACITY) {
    shim->order[shim->order_len++] = step;
  }
}

static void write_bytes(void *user, const uint8_t *bytes, size_t len) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)user;
  if (len <= TMX_TEST_TX_CAPACITY - shim->tx_len) {
    memcpy(&shim->tx[shim->tx_len], bytes, len);
    shim->tx_len += len;
  }
  if (shim->order_len == 0u || shim->order[shim->order_len - 1u] != 2u) {
    record(shim, 2u);
  }
}

static bool safe_stop(void *user) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)user;
  record(shim, 1u);
  return true;
}

static bool uart_tx_complete(void *user) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)user;
  record(shim, 3u);
  return shim->tx_complete;
}

static void enter_rom_bsl(void *user) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)user;
  record(shim, 4u);
  shim->enter_calls += 1u;
}

static bool prepare_flash(void *user,
                          const dctp_prepare_flash_request_t *request,
                          dctp_flash_transition_t *transition) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)user;
  return tmx_firmware_flash_prepare(&shim->flash, request, transition);
}

size_t tmx_flash_shim_size(void) { return sizeof(tmx_flash_test_shim_t); }

void *tmx_flash_shim_init(void *memory) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)memory;
  dctp_device_config_t config;
  tmx_firmware_flash_hooks_t hooks;
  if (shim == NULL) {
    return NULL;
  }
  memset(shim, 0, sizeof *shim);
  memset(&hooks, 0, sizeof hooks);
  hooks.safe_stop = safe_stop;
  hooks.uart_tx_complete = uart_tx_complete;
  hooks.enter_rom_bsl = enter_rom_bsl;
  hooks.user = shim;
  if (!tmx_firmware_flash_init(&shim->flash, &hooks)) {
    return NULL;
  }

  memset(&config, 0, sizeof config);
  memcpy(config.device_id, "TMX-MSPM0G3507!", DCTP_DEVICE_ID_LEN);
  config.boot_count = 1u;
  config.firmware_major = 1u;
  config.write = write_bytes;
  config.prepare_flash = prepare_flash;
  config.user = shim;
  return dctp_device_init(&shim->device, &config) ? shim : NULL;
}

void tmx_flash_shim_rx(void *memory, const uint8_t *bytes, size_t len,
                       uint32_t now_ms) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)memory;
  dctp_device_rx(&shim->device, bytes, len, now_ms);
}

size_t tmx_flash_shim_take_tx(void *memory, uint8_t *out, size_t capacity) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)memory;
  size_t len = shim->tx_len < capacity ? shim->tx_len : capacity;
  memcpy(out, shim->tx, len);
  shim->tx_len = 0u;
  return len;
}

void tmx_flash_shim_reset_order(void *memory) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)memory;
  shim->order_len = 0u;
}

void tmx_flash_shim_set_tx_complete(void *memory, int complete) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)memory;
  shim->tx_complete = complete != 0;
}

int tmx_flash_shim_poll_transition(void *memory) {
  tmx_flash_test_shim_t *shim = (tmx_flash_test_shim_t *)memory;
  return tmx_firmware_flash_poll_transition(&shim->flash, &shim->device) ? 1
                                                                        : 0;
}

size_t tmx_flash_shim_take_order(const void *memory, uint8_t *out,
                                 size_t capacity) {
  const tmx_flash_test_shim_t *shim = (const tmx_flash_test_shim_t *)memory;
  size_t len = shim->order_len < capacity ? shim->order_len : capacity;
  memcpy(out, shim->order, len);
  return len;
}

uint32_t tmx_flash_shim_enter_calls(const void *memory) {
  const tmx_flash_test_shim_t *shim = (const tmx_flash_test_shim_t *)memory;
  return shim->enter_calls;
}
