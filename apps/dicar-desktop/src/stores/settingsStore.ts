import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { SerialHardwareProfile } from "../domain/types";

type SettingsState = {
  serialHardwareProfile: SerialHardwareProfile;
  serialPortName: string;
  serialBaudRate: number;
  saveSerialConnection: (hardwareProfile: SerialHardwareProfile, portName: string, baudRate: number) => void;
};

export const useSettingsStore = create<SettingsState>()(persist((set) => ({
  serialHardwareProfile: "nanoUartWl",
  serialPortName: "",
  serialBaudRate: 460_800,
  saveSerialConnection: (serialHardwareProfile, serialPortName, serialBaudRate) => set({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
  }),
}), {
  name: "dicar-tune-settings",
  version: 1,
  migrate: (persisted) => {
    const legacy = persisted as Partial<SettingsState>;
    return {
      serialHardwareProfile: legacy.serialHardwareProfile ?? "nanoUartWl",
      serialPortName: legacy.serialPortName ?? "",
      serialBaudRate: legacy.serialBaudRate ?? 460_800,
    };
  },
  partialize: ({ serialHardwareProfile, serialPortName, serialBaudRate }) => ({ serialHardwareProfile, serialPortName, serialBaudRate }),
  merge: (persisted, current) => {
    const legacy = persisted as Partial<SettingsState>;
    return {
      ...current,
      serialHardwareProfile: legacy.serialHardwareProfile ?? current.serialHardwareProfile,
      serialPortName: legacy.serialPortName ?? current.serialPortName,
      serialBaudRate: legacy.serialBaudRate ?? current.serialBaudRate,
    };
  },
}));
