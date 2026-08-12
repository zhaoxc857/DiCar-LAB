import { parseVehicleProfile } from "./parser";

const header = `schema_version: 1
vehicle: { id: test-car, display_name: 测试车, type: 两轮差速, order: 10 }
`;

it("parses one bounded control loop without inventing device constraints", () => {
  expect(parseVehicleProfile(`${header}
control_loops:
  - id: speed
    label: 速度环
    category: 驱动控制
    hint: 对比目标和实际速度
    target_parameter: control.target_speed_mps
    gains: { Kp: pid.kp }
    telemetry: { feedback: drive.speed_mps, outputs: [motor.left_pwm] }
    recommended_channels: [drive.speed_mps, motor.left_pwm]
parameter_sections:
  - id: encoder
    label: 编码器
    parameters: [encoder.left.ppr]
scope_presets:
  - id: drive
    label: 驱动
    channels: [drive.speed_mps]
`)).toEqual({
    schemaVersion: 1,
    vehicle: { id: "test-car", displayName: "测试车", type: "两轮差速", order: 10 },
    controlLoops: [{
      id: "speed",
      label: "速度环",
      category: "驱动控制",
      hint: "对比目标和实际速度",
      targetParameter: "control.target_speed_mps",
      gains: { Kp: "pid.kp" },
      telemetry: { feedback: "drive.speed_mps", outputs: ["motor.left_pwm"] },
      recommendedChannels: ["drive.speed_mps", "motor.left_pwm"],
    }],
    parameterSections: [{ id: "encoder", label: "编码器", parameters: ["encoder.left.ppr"] }],
    scopePresets: [{ id: "drive", label: "驱动", channels: ["drive.speed_mps"] }],
  });
});

it.each([
  ["unknown schema", `schema_version: 2\nvehicle: { id: x, display_name: X, type: X, order: 1 }`, "schema_version"],
  ["unknown top-level key", `${header}control_loop: []`, "control_loop"],
  ["illegal id", `${header.replace("test-car", "Bad ID!")}`, "vehicle.id"],
  ["duplicate loop id", `${header}control_loops:\n  - { id: speed, label: A }\n  - { id: speed, label: B }`, "control_loops[1].id"],
  ["duplicate reference", `${header}scope_presets:\n  - { id: speed, label: A, channels: [drive.speed, drive.speed] }`, "scope_presets[0].channels[1]"],
  ["anchor", `${header}metadata: &common { note: x }`, "YAML 锚点"],
  ["alias", `${header}metadata: &common { note: x }\nextra: *common`, "YAML"],
  ["merge key", `${header}metadata:\n  base: &base { note: x }\n  merged: { <<: *base }`, "YAML merge key"],
  ["custom tag", `${header}metadata: !custom { note: x }`, "YAML 标签"],
])("rejects %s with a useful path", (_name, text, message) => {
  expect(() => parseVehicleProfile(text)).toThrow(message);
});

it("rejects text above 256 KiB before parsing it", () => {
  expect(() => parseVehicleProfile("x".repeat(256 * 1024 + 1))).toThrow("256 KiB");
  expect(() => parseVehicleProfile(`${header}metadata: ${"车".repeat(90_000)}`)).toThrow("256 KiB");
});

it("enforces collection and reference bounds", () => {
  const loops = Array.from({ length: 33 }, (_, index) => `  - { id: loop-${index}, label: Loop ${index} }`).join("\n");
  expect(() => parseVehicleProfile(`${header}control_loops:\n${loops}`)).toThrow("最多 32 个");
  const channels = Array.from({ length: 65 }, (_, index) => `channel.${index}`).join(", ");
  expect(() => parseVehicleProfile(`${header}scope_presets:\n  - { id: many, label: Many, channels: [${channels}] }`)).toThrow("最多 64 个");
});
