import type { ParameterSnapshot, TelemetryDescriptor } from "../domain/types";
import { parseVehicleProfile } from "./parser";
import { genericVehicleWorkspace, resolveVehicleWorkspace } from "./resolver";

const profile = parseVehicleProfile(`
schema_version: 1
vehicle: { id: test-car, display_name: 测试车, type: 差速, order: 1 }
control_loops:
  - id: speed
    label: 速度环
    target_parameter: control.target_speed
    gains: { Kp: pid.kp, Ki: pid.ki }
    telemetry: { target: drive.target, feedback: drive.speed, error: drive.error, outputs: [motor.pwm] }
    recommended_channels: [drive.target, drive.speed, missing.channel, motor.pwm]
parameter_sections:
  - { id: encoder, label: 编码器, parameters: [encoder.ppr, missing.param] }
scope_presets:
  - { id: drive, label: 驱动, channels: [motor.pwm, drive.speed, missing.channel] }
`);

const kp = parameter(1, "pid.kp", "f32", true);
const kiBool = parameter(2, "pid.ki", "bool", true);
const readOnlyTarget = parameter(4, "control.target_speed", "f32", false);
const ppr = parameter(100, "encoder.ppr", "u32", true);
const target = telemetry(207, "drive.target");
const speed = telemetry(200, "drive.speed");
const error = telemetry(208, "drive.error");
const pwm = telemetry(209, "motor.pwm", "u32");

it("binds exact parameter and telemetry machine names to stable numeric IDs", () => {
  const resolved = resolveVehicleWorkspace(profile, [kp, readOnlyTarget, ppr], [target, speed, error, pwm]);
  expect(resolved.controlLoops[0]).toMatchObject({
    id: "speed",
    gainParamIds: [{ label: "Kp", paramId: 1 }],
    targetParamId: 4,
    targetWritable: false,
    telemetry: { target: 207, feedback: 200, error: 208, outputs: [209] },
    recommendedChannelIds: [207, 200, 209],
  });
  expect(resolved.parameterSections[0].paramIds).toEqual([100]);
  expect(resolved.scopePresets[0].channelIds).toEqual([209, 200]);
});

it("reports read-only targets, non-numeric gains, missing references, and namespace mismatches", () => {
  const caseMismatch = parseVehicleProfile(`
schema_version: 1
vehicle: { id: mismatch, display_name: M, type: M, order: 1 }
control_loops:
  - { id: one, label: One, gains: { Kp: PID.KP, Ki: pid.ki }, target_parameter: drive.speed }
`);
  const resolved = resolveVehicleWorkspace(caseMismatch, [kp, kiBool], [speed]);
  expect(resolved.controlLoops).toHaveLength(0);
  expect(resolved.fallbackRequired).toBe(true);
  expect(resolved.issues).toEqual(expect.arrayContaining([
    expect.objectContaining({ severity: "error", path: "control_loops[0].gains.Kp" }),
    expect.objectContaining({ severity: "error", path: "control_loops[0].gains.Ki" }),
    expect.objectContaining({ severity: "error", path: "control_loops[0].target_parameter" }),
  ]));
});

it("keeps valid parts of a partially compatible loop and emits deterministic issues", () => {
  const resolved = resolveVehicleWorkspace(profile, [kp, kiBool, readOnlyTarget, ppr], [speed]);
  expect(resolved.controlLoops).toHaveLength(1);
  expect(resolved.controlLoops[0].recommendedChannelIds).toEqual([200]);
  expect(resolved.issues[0]).toMatchObject({ path: "control_loops[0].target_parameter", severity: "warning" });
  expect(resolved.issues.map(({ path }) => path)).toContain("control_loops[0].gains.Ki");
  expect(resolved.issues.map(({ path }) => path)).toContain("parameter_sections[0].parameters[1]");
});

it("creates a generic workspace that keeps every Manifest parameter and telemetry channel reachable", () => {
  const generic = genericVehicleWorkspace([kp, ppr], [speed, pwm]);
  expect(generic).toMatchObject({ profileId: "generic-manifest", fallbackRequired: false });
  expect(generic.parameterSections.map(({ id, paramIds }) => ({ id, paramIds }))).toEqual([
    { id: "control", paramIds: [1] },
    { id: "encoder", paramIds: [100] },
  ]);
  expect(generic.scopePresets[0]).toMatchObject({ id: "all", channelIds: [200, 209] });
});

function parameter(paramId: number, machineName: string, kind: "f32" | "u32" | "bool", writable: boolean): ParameterSnapshot {
  const value = kind === "bool" ? { kind, value: false } as const : { kind, value: 1 } as const;
  return { paramId, machineName, displayName: machineName, group: machineName.startsWith("encoder") ? "Encoder" : "Control", unit: "", ramValue: value, persistedValue: value, revision: 1, dirty: false, syncKnown: true, writeState: "idle", writable, dangerous: false, lastError: null };
}

function telemetry(channelId: number, machineName: string, telemetryType: TelemetryDescriptor["telemetryType"] = "f32"): TelemetryDescriptor {
  return { channelId, machineName, displayName: machineName, group: "Drive", unit: "", telemetryType };
}
