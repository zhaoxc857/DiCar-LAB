#include "tmx_firmware_flash.h"

#include <ti/driverlib/driverlib.h>

void tmx_mspm0_sdk_enter_rom_bsl(void *user) {
  (void)user;
  DL_SYSCTL_resetDevice(DL_SYSCTL_RESET_BOOTLOADER_ENTRY);

  /* DriverLib 声明复位立即生效；若硬件异常未复位，禁止回到正常控制流。 */
  for (;;) {
  }
}
