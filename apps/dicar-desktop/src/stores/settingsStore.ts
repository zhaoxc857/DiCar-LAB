import { create } from "zustand";
import { persist } from "zustand/middleware";
import { DEFAULT_AI_MODEL } from "../ai/aiClient";
import type { SerialHardwareProfile } from "../domain/types";

const SETTINGS_STORAGE_KEY = "dicar-tune-settings";

export type WorkbenchMode = "standard" | "track";

type PersistedSettingsV4 = {
  serialHardwareProfile: SerialHardwareProfile;
  serialPortName: string;
  serialBaudRate: number;
  aiModel: string;
  workbenchMode: WorkbenchMode;
};

type SettingsState = PersistedSettingsV4 & {
  saveSerialConnection: (hardwareProfile: SerialHardwareProfile, portName: string, baudRate: number) => void;
  saveAiModel: (model: string) => void;
  saveWorkbenchMode: (mode: WorkbenchMode) => void;
};

function normalizeWorkbenchMode(value: unknown): WorkbenchMode {
  return value === "track" ? "track" : "standard";
}

export function migrateSettingsV4(persisted: unknown): PersistedSettingsV4 {
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
    workbenchMode: normalizeWorkbenchMode(legacy.workbenchMode),
  };
}

export function scrubLegacyAiSettings(storage: Storage): void {
  const raw = storage.getItem(SETTINGS_STORAGE_KEY);
  if (raw === null) return;
  try {
    const envelope = JSON.parse(raw) as { state?: unknown };
    storage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify({
      state: migrateSettingsV4(envelope.state),
      version: 4,
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
  workbenchMode: "standard",
  saveSerialConnection: (serialHardwareProfile, serialPortName, serialBaudRate) => set({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
  }),
  saveAiModel: (aiModel) => set({ aiModel: aiModel.trim() || DEFAULT_AI_MODEL }),
  saveWorkbenchMode: (workbenchMode) => set({ workbenchMode }),
}), {
  name: SETTINGS_STORAGE_KEY,
  version: 4,
  migrate: migrateSettingsV4,
  partialize: ({ serialHardwareProfile, serialPortName, serialBaudRate, aiModel, workbenchMode }) => ({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
    aiModel,
    workbenchMode,
  }),
  merge: (persisted, current) => ({
    ...current,
    ...migrateSettingsV4(persisted),
  }),
}));
