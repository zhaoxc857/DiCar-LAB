import type { TelemetryDescriptor } from "../domain/types";
import { buildTelemetryWorkgroups, clipWorkgroup, mergeTelemetryWorkgroups, namespaceProfileWorkgroups } from "./telemetryWorkgroups";

const descriptors: TelemetryDescriptor[] = [
  descriptor(20, "motor.left_rpm", "左轮转速", "rpm"),
  descriptor(10, "encoder.left_speed", "左编码器速度", "m/s"),
  descriptor(30, "imu.yaw_rate", "横摆角速度", "deg/s"),
  descriptor(40, "battery.voltage", "电池电压", "V"),
  descriptor(50, "tracking.error", "循迹误差", "m"),
];

it("derives non-empty semantic workgroups in Manifest order and allows overlap", () => {
  expect(buildTelemetryWorkgroups(descriptors)).toEqual([
    { id: "speed", label: "速度", channelIds: [20, 10, 30] },
    { id: "heading", label: "航向与姿态", channelIds: [30] },
    { id: "encoder", label: "编码器", channelIds: [10] },
    { id: "motor", label: "电机", channelIds: [20] },
    { id: "power", label: "电源", channelIds: [40] },
    { id: "all", label: "全部通道", channelIds: [20, 10, 30, 40, 50] },
  ]);
});

it("namespaces valid profile preset ids away from automatic ids", () => {
  expect(namespaceProfileWorkgroups([{ id: "speed", label: "车型速度", channelIds: [20] }])).toEqual([{ id: "profile:speed", label: "车型速度", channelIds: [20] }]);
});

it("omits empty semantic groups but keeps All Channels for an unclassified Manifest", () => {
  expect(buildTelemetryWorkgroups([descriptor(7, "tracking.error", "循迹误差", "m")])).toEqual([
    { id: "all", label: "全部通道", channelIds: [7] },
  ]);
  expect(buildTelemetryWorkgroups([])).toEqual([]);
});

it("clips in workgroup order and reports every omitted channel", () => {
  expect(clipWorkgroup({ id: "all", label: "全部通道", channelIds: [20, 10, 30, 40, 50] }, 3)).toEqual({
    channelIds: [20, 10, 30],
    omittedCount: 2,
  });
});

it("puts profile presets first, rejects duplicate ids, and filters unavailable channels", () => {
  const automatic = buildTelemetryWorkgroups(descriptors);
  expect(mergeTelemetryWorkgroups([{ id: "profile-drive", label: "车型驱动", channelIds: [50, 999, 20, 20] }], automatic, descriptors.map(({ channelId }) => channelId))[0]).toEqual({ id: "profile-drive", label: "车型驱动", channelIds: [50, 20] });
  expect(() => mergeTelemetryWorkgroups([{ id: "speed", label: "冲突", channelIds: [20] }], automatic, descriptors.map(({ channelId }) => channelId))).toThrow(/重复工作组 ID speed/);
});

function descriptor(channelId: number, machineName: string, displayName: string, unit: string): TelemetryDescriptor {
  return { channelId, telemetryType: "f32", machineName, displayName, group: "测试", unit };
}
