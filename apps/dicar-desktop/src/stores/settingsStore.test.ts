import {
  migrateSettingsV4,
  scrubLegacyAiSettings,
  useSettingsStore,
} from "./settingsStore";

beforeEach(() => {
  localStorage.clear();
  useSettingsStore.setState({ workbenchMode: "standard" });
});

it("adds standard workbench mode while migrating without carrying AI secrets", () => {
  const migrated = migrateSettingsV4({
    serialHardwareProfile: "nanoUartWl",
    serialPortName: "COM7",
    serialBaudRate: 115_200,
    aiBaseUrl: "https://attacker.invalid",
    aiModel: "deepseek-reasoner",
    aiApiKey: "sk-plaintext",
  });

  expect(migrated).toEqual({
    serialHardwareProfile: "nanoUartWl",
    serialPortName: "COM7",
    serialBaudRate: 115_200,
    aiModel: "deepseek-reasoner",
    workbenchMode: "standard",
  });
  expect(JSON.stringify(migrated)).not.toContain("sk-plaintext");
  expect(JSON.stringify(migrated)).not.toContain("attacker.invalid");
});

it("rejects an unknown workbench mode during migration", () => {
  expect(migrateSettingsV4({ workbenchMode: "track" }).workbenchMode).toBe("track");
  expect(migrateSettingsV4({ workbenchMode: "invalid" }).workbenchMode).toBe("standard");
});

it("persists track mode without introducing plaintext AI settings", () => {
  useSettingsStore.getState().saveWorkbenchMode("track");

  const raw = localStorage.getItem("dicar-tune-settings") ?? "";
  expect(raw).toContain('"workbenchMode":"track"');
  expect(raw).not.toContain("aiApiKey");
  expect(raw).not.toContain("aiBaseUrl");
});

it("scrubs legacy plaintext fields from the raw localStorage payload", () => {
  localStorage.setItem("dicar-tune-settings", JSON.stringify({
    state: {
      serialPortName: "COM7",
      aiBaseUrl: "https://api.deepseek.com",
      aiModel: "deepseek-chat",
      aiApiKey: "sk-plaintext",
    },
    version: 2,
  }));

  scrubLegacyAiSettings(localStorage);

  const raw = localStorage.getItem("dicar-tune-settings") ?? "";
  expect(raw).not.toContain("sk-plaintext");
  expect(raw).not.toContain("aiApiKey");
  expect(raw).not.toContain("aiBaseUrl");
  expect(JSON.parse(raw)).toMatchObject({
    version: 4,
    state: {
      serialPortName: "COM7",
      aiModel: "deepseek-chat",
      workbenchMode: "standard",
    },
  });
});

it("removes a corrupt legacy payload rather than risking secret retention", () => {
  localStorage.setItem("dicar-tune-settings", "not-json-sk-plaintext");

  scrubLegacyAiSettings(localStorage);

  expect(localStorage.getItem("dicar-tune-settings")).toBeNull();
});
