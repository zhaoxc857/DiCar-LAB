import { create } from "zustand";

type SettingsState = {
  vehicleId: string;
  setVehicleId: (vehicleId: string) => void;
};

export const useSettingsStore = create<SettingsState>((set) => ({
  vehicleId: "car-01",
  setVehicleId: (vehicleId) => set({ vehicleId }),
}));
