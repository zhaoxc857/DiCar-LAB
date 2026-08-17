#include "tmx_firmware_flash.h"

#include <string.h>

bool tmx_firmware_flash_init(tmx_firmware_flash_t *adapter,
                             const tmx_firmware_flash_hooks_t *hooks) {
  if (adapter == NULL || hooks == NULL || hooks->safe_stop == NULL ||
      hooks->uart_tx_complete == NULL || hooks->enter_rom_bsl == NULL) {
    return false;
  }
  memset(adapter, 0, sizeof *adapter);
  adapter->hooks = *hooks;
  return true;
}

bool tmx_firmware_flash_prepare(
    tmx_firmware_flash_t *adapter,
    const dctp_prepare_flash_request_t *request,
    dctp_flash_transition_t *transition) {
  if (adapter == NULL || request == NULL || transition == NULL ||
      adapter->armed ||
      request->target_id != DCTP_FIRMWARE_TARGET_LCKFB_TMX_MSPM0G3507 ||
      request->image_len < TMX_FIRMWARE_FLASH_MIN_IMAGE_LEN ||
      request->image_len > TMX_FIRMWARE_FLASH_MAX_IMAGE_LEN) {
    return false;
  }
  if (!adapter->hooks.safe_stop(adapter->hooks.user)) {
    return false;
  }

  memcpy(transition->operation_id, request->operation_id,
         DCTP_FLASH_OPERATION_ID_LEN);
  transition->bootloader_protocol = DCTP_BOOTLOADER_TI_MSPM0_ROM_BSL_UART;
  transition->entry_delay_ms = TMX_FIRMWARE_FLASH_ENTRY_DELAY_MS;
  transition->initial_baud = TMX_FIRMWARE_FLASH_BAUD;
  adapter->armed = true;
  return true;
}

bool tmx_firmware_flash_poll_transition(tmx_firmware_flash_t *adapter,
                                        dctp_device_t *device) {
  dctp_flash_transition_t transition;
  if (adapter == NULL || device == NULL || !adapter->armed) {
    return false;
  }
  if (!adapter->hooks.uart_tx_complete(adapter->hooks.user)) {
    return false;
  }
  if (!dctp_device_take_flash_transition(device, &transition)) {
    return false;
  }

  adapter->armed = false;
  if (transition.bootloader_protocol !=
          DCTP_BOOTLOADER_TI_MSPM0_ROM_BSL_UART ||
      transition.entry_delay_ms != TMX_FIRMWARE_FLASH_ENTRY_DELAY_MS ||
      transition.initial_baud != TMX_FIRMWARE_FLASH_BAUD) {
    return false;
  }
  adapter->hooks.enter_rom_bsl(adapter->hooks.user);
  return true;
}
