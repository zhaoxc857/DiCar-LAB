import type { ParameterSnapshot, ParameterValue, TelemetryDescriptor } from "../domain/types";
import { builtInProfiles, GENERIC_PROFILE_ID } from "../vehicleProfiles/catalog";
import { resolveVehicleWorkspace } from "../vehicleProfiles/resolver";
import { VEHICLE_PROFILE_STORAGE_KEY, useVehicleProfileStore } from "./vehicleProfileStore";

const USER_YAML = profileYaml("user-car", "用户车");
const UPDATED_USER_YAML = profileYaml("user-car", "更新后的用户车");
const BUILTIN_ID_YAML = profileYaml("dicar-diff-drive", "冒充内置车型");

beforeEach(() => {
  localStorage.clear();
  useVehicleProfileStore.getState().reset();
});

it("packages a built-in profile that resolves useful simulator tasks", () => {
  const builtIn = builtInProfiles.find(({ profile }) => profile.vehicle.id === "dicar-diff-drive");
  expect(builtIn).toBeDefined();
  const resolved = resolveVehicleWorkspace(builtIn!.profile, simulatorParameters(), simulatorTelemetry());
  expect(resolved.controlLoops.map(({ id }) => id)).toEqual(["speed"]);
  expect(resolved.controlLoops[0].recommendedChannelIds).toEqual([207, 200, 208, 209, 210]);
  expect(resolved.parameterSections.map(({ id }) => id)).toEqual(["encoder", "drive"]);
});

it("requires explicit replacement and never lets a user shadow a built-in id", () => {
  const store = useVehicleProfileStore.getState();
  expect(store.importProfile(USER_YAML, false).status).toBe("imported");
  expect(useVehicleProfileStore.getState().importProfile(UPDATED_USER_YAML, false).status).toBe("needsReplace");
  expect(useVehicleProfileStore.getState().importProfile(UPDATED_USER_YAML, true).status).toBe("imported");
  expect(useVehicleProfileStore.getState().userProfiles[0].profile.vehicle.displayName).toBe("更新后的用户车");
  expect(useVehicleProfileStore.getState().importProfile(BUILTIN_ID_YAML, true).status).toBe("failed");
});

it("removing the active user profile falls back to generic", () => {
  const store = useVehicleProfileStore.getState();
  store.importProfile(USER_YAML, false);
  useVehicleProfileStore.getState().selectProfile("user-car");
  useVehicleProfileStore.getState().removeUserProfile("user-car");
  expect(useVehicleProfileStore.getState().selectedProfileId).toBe(GENERIC_PROFILE_ID);
});

it("enforces the 16-profile and 2-MiB aggregate limits", () => {
  for (let index = 0; index < 16; index += 1) {
    expect(useVehicleProfileStore.getState().importProfile(profileYaml(`car-${index}`, `车辆 ${index}`), false).status).toBe("imported");
  }
  expect(useVehicleProfileStore.getState().importProfile(profileYaml("car-17", "车辆 17"), false)).toMatchObject({ status: "failed", message: expect.stringContaining("16") });

  useVehicleProfileStore.getState().reset();
  for (let index = 0; index < 8; index += 1) {
    const padded = `${profileYaml(`large-${index}`, `大配置 ${index}`)}\n#${"x".repeat(255_000)}`;
    expect(useVehicleProfileStore.getState().importProfile(padded, false).status).toBe("imported");
  }
  const overflow = `${profileYaml("large-8", "大配置 8")}\n#${"x".repeat(255_000)}`;
  expect(useVehicleProfileStore.getState().importProfile(overflow, false)).toMatchObject({ status: "failed", message: expect.stringContaining("2 MiB") });
});

it("persists only selected id and YAML text", () => {
  useVehicleProfileStore.getState().importProfile(USER_YAML, false);
  useVehicleProfileStore.getState().selectProfile("user-car");
  const persisted = JSON.parse(localStorage.getItem(VEHICLE_PROFILE_STORAGE_KEY) ?? "null") as Record<string, unknown>;
  expect(persisted).toEqual({ selectedProfileId: "user-car", userYamlTexts: [USER_YAML] });
  expect(JSON.stringify(persisted)).not.toContain("schemaVersion");
});

it("keeps the previous in-memory state when persistence fails", () => {
  const setItem = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new DOMException("quota", "QuotaExceededError"); });
  expect(useVehicleProfileStore.getState().importProfile(USER_YAML, false)).toMatchObject({ status: "failed", message: expect.stringContaining("保存") });
  expect(useVehicleProfileStore.getState().userProfiles).toEqual([]);
  expect(useVehicleProfileStore.getState().selectedProfileId).toBe(GENERIC_PROFILE_ID);
  setItem.mockRestore();
});

it("migrates legacy settings without changing serial fields", async () => {
  localStorage.setItem("dicar-tune-settings", JSON.stringify({
    state: { vehicleId: "car-01", serialHardwareProfile: "hc05", serialPortName: "COM9", serialBaudRate: 115_200 },
    version: 0,
  }));
  vi.resetModules();
  const { useSettingsStore } = await import("./settingsStore");
  expect(useSettingsStore.getState()).toMatchObject({ serialHardwareProfile: "hc05", serialPortName: "COM9", serialBaudRate: 115_200 });
  expect(useSettingsStore.getState()).not.toHaveProperty("vehicleId");
  expect(useVehicleProfileStore.getState().selectedProfileId).toBe(GENERIC_PROFILE_ID);
});

function profileYaml(id: string, displayName: string): string {
  return `schema_version: 1\nvehicle: { id: ${id}, display_name: ${displayName}, type: 测试, order: 50 }\n`;
}

function simulatorParameters(): ParameterSnapshot[] {
  return [
    parameter(1, "pid.kp", "f32"), parameter(100, "encoder.left.ppr", "u32"), parameter(101, "encoder.right.ppr", "u32"),
    parameter(102, "encoder.quadrature_multiplier", "enum"), parameter(105, "encoder.left.inverted", "bool"), parameter(106, "encoder.right.inverted", "bool"),
    parameter(107, "drive.wheel_diameter_mm", "f32"), parameter(108, "drive.gear_ratio", "f32"), parameter(109, "encoder.sample_period_us", "u32"),
    parameter(110, "encoder.speed_lpf_hz", "f32"), parameter(111, "encoder.jump_threshold_counts", "u32"), parameter(112, "encoder.max_credible_rpm", "f32"),
    parameter(113, "encoder.missing_pulse_detection", "bool"),
  ];
}

function simulatorTelemetry(): TelemetryDescriptor[] {
  const names = ["drive.speed_mps", "encoder.left_delta", "encoder.left_total", "drive.fault_flags", "encoder.right_total", "drive.left_wheel_speed_mps", "drive.right_wheel_speed_mps", "drive.target_speed_mps", "drive.speed_error_mps", "motor.left_pwm", "motor.right_pwm"];
  return names.map((machineName, index) => ({ channelId: 200 + index, machineName, displayName: machineName, group: "模拟", unit: "", telemetryType: index === 3 ? "flags32" : "f32" }));
}

function parameter(paramId: number, machineName: string, kind: ParameterValue["kind"]): ParameterSnapshot {
  const value = kind === "bool" ? { kind, value: false } as const : kind === "enum" ? { kind, value: 1 } as const : { kind, value: 1 } as ParameterValue;
  return { paramId, machineName, displayName: machineName, group: "模拟", unit: "", ramValue: value, persistedValue: value, revision: 1, dirty: false, syncKnown: true, writeState: "idle", writable: true, dangerous: false, lastError: null };
}
