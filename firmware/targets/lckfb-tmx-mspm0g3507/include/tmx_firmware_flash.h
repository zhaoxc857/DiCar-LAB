#ifndef TMX_FIRMWARE_FLASH_H
#define TMX_FIRMWARE_FLASH_H

#include <stdbool.h>
#include <stdint.h>

#include "dctp_device.h"

#ifdef __cplusplus
extern "C" {
#endif

enum {
  TMX_FIRMWARE_FLASH_ENTRY_DELAY_MS = 250u,
  TMX_FIRMWARE_FLASH_BAUD = 9600u,
  TMX_FIRMWARE_FLASH_MIN_IMAGE_LEN = 1024u,
  TMX_FIRMWARE_FLASH_MAX_IMAGE_LEN = 131072u,
};

typedef struct {
  bool (*safe_stop)(void *user);
  bool (*uart_tx_complete)(void *user);
  void (*enter_rom_bsl)(void *user);
  void *user;
} tmx_firmware_flash_hooks_t;

typedef struct {
  tmx_firmware_flash_hooks_t hooks;
  bool armed;
} tmx_firmware_flash_t;

bool tmx_firmware_flash_init(tmx_firmware_flash_t *adapter,
                             const tmx_firmware_flash_hooks_t *hooks);

bool tmx_firmware_flash_prepare(
    tmx_firmware_flash_t *adapter,
    const dctp_prepare_flash_request_t *request,
    dctp_flash_transition_t *transition);

/*
 * 在主循环调用。只有 PREPARE_FLASH ACK 已完整离开发送路径后才消费一次性
 * transition 并调用 enter_rom_bsl；中断上下文不得调用本函数。
 */
bool tmx_firmware_flash_poll_transition(tmx_firmware_flash_t *adapter,
                                        dctp_device_t *device);

/*
 * 可直接赋给 hooks.enter_rom_bsl 的 MSPM0 DriverLib 实现。
 * 需要 MSPM0 SDK 工程定义 __MSPM0G3507__ 并链接对应 DriverLib。
 * 复位请求按 TI ROM BSL 的 BOOTLOADER_ENTRY 级别发出；正常情况下不返回。
 */
void tmx_mspm0_sdk_enter_rom_bsl(void *user);

#ifdef __cplusplus
}
#endif

#endif
