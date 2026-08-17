#ifndef TMX_TEST_TI_DRIVERLIB_H
#define TMX_TEST_TI_DRIVERLIB_H

#include <stdint.h>

#define DL_SYSCTL_RESET_BOOTLOADER_ENTRY UINT32_C(0x00000003)

static inline void DL_SYSCTL_resetDevice(uint32_t reset_type) {
  (void)reset_type;
}

#endif
