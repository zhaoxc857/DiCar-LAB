# 天猛星 MSPM0G3507 无线固件切换适配

本目录把通用 `dctp-device` 的 `PREPARE_FLASH` 回调接到天猛星应用的安全停机与
TI ROM BSL 入口。首版固定使用 UART0 PA10/PA11、9600 8N1；HC-05 TX 接
PA11/BSL_RX，HC-05 RX 接 PA10/BSL_TX。板载 CH340 与无线模块不得同时驱动
这组引脚。

应用需要提供三个钩子：

- `safe_stop`：关闭电机和高功率输出，停止遥测并锁定参数写入；失败返回 false。
- `uart_tx_complete`：只有 PREPARE_FLASH_ACK 已完全离开 UART 时才返回 true。
- `enter_rom_bsl`：设置 TI ROM BSL 启动入口并触发系统复位；不得提前调用。

把 `tmx_firmware_flash_prepare` 包装为 `dctp_device_config_t.prepare_flash`。每次主
循环发送完成后调用 `tmx_firmware_flash_poll_transition`。该函数只消费一次 DCTP
transition，因此重传不会造成二次复位。

SDK 工程同时加入 `src/tmx_mspm0_sdk_entry.c` 后，可直接设置：

```c
hooks.enter_rom_bsl = tmx_mspm0_sdk_enter_rom_bsl;
```

该实现使用 MSPM0 SDK DriverLib 的
`DL_SYSCTL_resetDevice(DL_SYSCTL_RESET_BOOTLOADER_ENTRY)`。工程必须选择
MSPM0G3507（编译宏 `__MSPM0G3507__`）；本仓库主机测试用同签名的 DriverLib
替身做编译检查。本机没有 `tiarmclang`/`arm-none-eabi-gcc`，因此真实 SDK 的
ARM 交叉编译、NONMAIN BSL 配置/密码写入和 PA10/PA11 实板通信仍须在天猛星上
核对后才能作为发布验证，当前结果不代表实板烧录已经验证。

TI Arm Clang 语法检查示例（安装工具链并设置 SDK 路径后）：

```powershell
$mspm0Sdk = 'C:\ti\mspm0_sdk_2_11_00_07'
tiarmclang -mcpu=cortex-m0plus -mthumb -std=c99 -D__MSPM0G3507__ `
  -I "$mspm0Sdk\source" -I 'firmware\dctp-device\include' `
  -I 'firmware\targets\lckfb-tmx-mspm0g3507\include' `
  -c 'firmware\targets\lckfb-tmx-mspm0g3507\src\tmx_mspm0_sdk_entry.c'
```

Arm GNU 工具链使用相同的设备宏和 include 路径：

```powershell
$mspm0Sdk = 'C:\ti\mspm0_sdk_2_11_00_07'
arm-none-eabi-gcc -mcpu=cortex-m0plus -mthumb -std=c99 -D__MSPM0G3507__ `
  -I "$mspm0Sdk\source" -I 'firmware\dctp-device\include' `
  -I 'firmware\targets\lckfb-tmx-mspm0g3507\include' `
  -c 'firmware\targets\lckfb-tmx-mspm0g3507\src\tmx_mspm0_sdk_entry.c'
```

命令只示范目标入口的编译参数；完整固件仍须把 `dctp-device`、
`tmx_firmware_flash.c`、SysConfig 生成文件、链接脚本和对应 DriverLib 一并加入工程。
本机两种 ARM 编译器均未安装，所以这些命令尚未在当前环境执行成功。

主机交叉测试：

```powershell
cargo test -p dctp-device-c --test target_flash_adapter
```
