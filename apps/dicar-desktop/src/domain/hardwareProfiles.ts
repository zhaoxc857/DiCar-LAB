import type { SerialHardwareProfile } from "./types";

export type HardwareProfileDefinition = {
  label: string;
  recommendedBaudRate: number;
  probeBaudRates: readonly number[];
  guidance: readonly string[];
  warning: string | null;
};

export const SUPPORTED_SERIAL_BAUD_RATES = [
  9_600,
  38_400,
  57_600,
  115_200,
  230_400,
  460_800,
  921_600,
] as const;

export const HARDWARE_PROFILES: Record<SerialHardwareProfile, HardwareProfileDefinition> = {
  nanoUartWl: {
    label: "nanoUART-wl",
    recommendedBaudRate: 460_800,
    probeBaudRates: [460_800, 230_400, 115_200],
    guidance: ["电脑端插入 USB 后选择新增 COM", "车端连接 3V3、GND，并将 TX/RX 交叉连接"],
    warning: null,
  },
  hc05BluetoothSpp: {
    label: "HC-05 蓝牙串口",
    recommendedBaudRate: 115_200,
    probeBaudRates: [115_200, 9_600, 38_400, 57_600, 230_400, 460_800],
    guidance: ["先在 Windows 蓝牙设置中完成配对", "请选择系统创建的传出（Outgoing）COM 口", "车端 TX/RX 交叉连接并与 MCU 共地"],
    warning: "HC-05 UART 按 3.3V 逻辑处理；5V MCU 发往 HC-05 RX 时必须分压或进行电平转换。",
  },
  genericSerial: {
    label: "通用串口",
    recommendedBaudRate: 115_200,
    probeBaudRates: [115_200],
    guidance: ["选择设备实际提供的 COM 口和与车端 MCU 一致的波特率"],
    warning: "连接前请确认串口 IO 电平与目标板兼容。",
  },
};
