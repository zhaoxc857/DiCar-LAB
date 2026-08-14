import { create } from "zustand";
import { persist } from "zustand/middleware";
import { DEFAULT_AI_MODEL } from "../ai/aiClient";
import type { SerialHardwareProfile } from "../domain/types";

const SETTINGS_STORAGE_KEY = "dicar-tune-settings";

type PersistedSettingsV3 = {
  serialHardwareProfile: SerialHardwareProfile;
  serialPortName: string;
  serialBaudRate: number;
  aiModel: string;
};

type SettingsState = PersistedSettingsV3 & {
  saveSerialConnection: (hardwareProfile: SerialHardwareProfile, portName: string, baudRate: number) => void;
  saveAiModel: (model: string) => void;
};

export function migrateSettingsV3(persisted: unknown): PersistedSettingsV3 {
  const legacy = (typeof persisted === "object" && persisted !== null ? persisted : {}) as Record<string, unknown>;
  const serialBaudRate = legacy.serialBaudRate;
  const aiModel = legacy.aiModel;
  return {
    serialHardwareProfile: typeof legacy.serialHardwareProfile === "string"
      ? legacy.serialHardwareProfile as SerialHardwareProfile
      : "nanoUartWl",
    serialPortName: typeof legacy.serialPortName === "string" ? legacy.serialPortName : "",
    serialBaudRate: typeof serialBaudRate === "number" && Number.isFinite(serialBaudRate)
      ? serialBaudRate
      : 460_800,
    aiModel: typeof aiModel === "string" && aiModel.trim().length > 0 ? aiModel.trim() : DEFAULT_AI_MODEL,
  };
}

export function scrubLegacyAiSettings(storage: Storage): void {
  const raw = storage.getItem(SETTINGS_STORAGE_KEY);
  if (raw === null) return;
  try {
    const envelope = JSON.parse(raw) as { state?: unknown };
    storage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify({
      state: migrateSettingsV3(envelope.state),
      version: 3,
    }));
  } catch {
    storage.removeItem(SETTINGS_STORAGE_KEY);
  }
}

if (typeof window !== "undefined") {
  try {
    scrubLegacyAiSettings(window.localStorage);
  } catch {
    // localStorage can be denied by browser policy; Zustand will surface its own persistence fallback.
  }
}

export const useSettingsStore = create<SettingsState>()(persist((set) => ({
  serialHardwareProfile: "nanoUartWl",
  serialPortName: "",
  serialBaudRate: 460_800,
  aiModel: DEFAULT_AI_MODEL,
  saveSerialConnection: (serialHardwareProfile, serialPortName, serialBaudRate) => set({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
  }),
  saveAiModel: (aiModel) => set({ aiModel: aiModel.trim() || DEFAULT_AI_MODEL }),
}), {
  name: SETTINGS_STORAGE_KEY,
  version: 3,
  migrate: migrateSettingsV3,
  partialize: ({ serialHardwareProfile, serialPortName, serialBaudRate, aiModel }) => ({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
    aiModel,
  }),
  merge: (persisted, current) => ({
    ...current,
    ...migrateSettingsV3(persisted),
  }),
}));
