import { migrateSettingsV3, scrubLegacyAiSettings } from "./settingsStore";

beforeEach(() => {
  localStorage.clear();
});

it("migrates settings to v3 without carrying a base URL or plaintext API key", () => {
  const migrated = migrateSettingsV3({
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
  });
  expect(JSON.stringify(migrated)).not.toContain("sk-plaintext");
  expect(JSON.stringify(migrated)).not.toContain("attacker.invalid");
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
    version: 3,
    state: { serialPortName: "COM7", aiModel: "deepseek-chat" },
  });
});

it("removes a corrupt legacy payload rather than risking secret retention", () => {
  localStorage.setItem("dicar-tune-settings", "not-json-sk-plaintext");

  scrubLegacyAiSettings(localStorage);

  expect(localStorage.getItem("dicar-tune-settings")).toBeNull();
});
