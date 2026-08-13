import { create } from "zustand";
import { persist } from "zustand/middleware";
import { DEFAULT_AI_BASE_URL, DEFAULT_AI_MODEL } from "../ai/aiClient";
import type { SerialHardwareProfile } from "../domain/types";

type SettingsState = {
  serialHardwareProfile: SerialHardwareProfile;
  serialPortName: string;
  serialBaudRate: number;
  aiBaseUrl: string;
  aiModel: string;
  /** 只保存在本机 localStorage，不随任何数据上传。 */
  aiApiKey: string;
  saveSerialConnection: (hardwareProfile: SerialHardwareProfile, portName: string, baudRate: number) => void;
  saveAiSettings: (baseUrl: string, model: string, apiKey: string) => void;
};

const AI_DEFAULTS = { aiBaseUrl: DEFAULT_AI_BASE_URL, aiModel: DEFAULT_AI_MODEL, aiApiKey: "" };

export const useSettingsStore = create<SettingsState>()(persist((set) => ({
  serialHardwareProfile: "nanoUartWl",
  serialPortName: "",
  serialBaudRate: 460_800,
  ...AI_DEFAULTS,
  saveSerialConnection: (serialHardwareProfile, serialPortName, serialBaudRate) => set({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
  }),
  saveAiSettings: (aiBaseUrl, aiModel, aiApiKey) => set({
    aiBaseUrl: aiBaseUrl.trim() || DEFAULT_AI_BASE_URL,
    aiModel: aiModel.trim() || DEFAULT_AI_MODEL,
    aiApiKey: aiApiKey.trim(),
  }),
}), {
  name: "dicar-tune-settings",
  version: 2,
  migrate: (persisted) => {
    const legacy = persisted as Partial<SettingsState>;
    return {
      serialHardwareProfile: legacy.serialHardwareProfile ?? "nanoUartWl",
      serialPortName: legacy.serialPortName ?? "",
      serialBaudRate: legacy.serialBaudRate ?? 460_800,
      aiBaseUrl: legacy.aiBaseUrl ?? AI_DEFAULTS.aiBaseUrl,
      aiModel: legacy.aiModel ?? AI_DEFAULTS.aiModel,
      aiApiKey: legacy.aiApiKey ?? "",
    };
  },
  partialize: ({ serialHardwareProfile, serialPortName, serialBaudRate, aiBaseUrl, aiModel, aiApiKey }) => ({
    serialHardwareProfile,
    serialPortName,
    serialBaudRate,
    aiBaseUrl,
    aiModel,
    aiApiKey,
  }),
  merge: (persisted, current) => {
    const legacy = persisted as Partial<SettingsState>;
    return {
      ...current,
      serialHardwareProfile: legacy.serialHardwareProfile ?? current.serialHardwareProfile,
      serialPortName: legacy.serialPortName ?? current.serialPortName,
      serialBaudRate: legacy.serialBaudRate ?? current.serialBaudRate,
      aiBaseUrl: legacy.aiBaseUrl ?? current.aiBaseUrl,
      aiModel: legacy.aiModel ?? current.aiModel,
      aiApiKey: legacy.aiApiKey ?? current.aiApiKey,
    };
  },
}));
