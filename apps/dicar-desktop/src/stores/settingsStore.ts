import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { SerialHardwareProfile } from "../domain/types";

type SettingsState = {
  vehicleId: string;
  serialHardwareProfile: SerialHardwareProfile;
  serialPortName: string;
  serialBaudRate: number;
  setVehicleId: (vehicleId: string) => void;
  saveSerialConnection: (hardwareProfile: SerialHardwareProfile, portName: string, baudRate: number) => void;
};

export const useSettingsStore = create<SettingsState>()(persist((set) => ({
  vehicleId: "car-01",
  serialHardwareProfile: "nanoUartWl",
  serialPortName: "",
  serialBaudRate: 460_800,
  setVehicleId: (vehicleId) => set({ vehicleId }),
  saveSerialConnection: (serialHardwareProfile, serialPortName, serialBaudRate) => set({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
  }),
}), { name: "dicar-tune-settings" }));
