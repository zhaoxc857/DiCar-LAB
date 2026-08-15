import { render, screen } from "@testing-library/react";
import { MockBridge } from "../../bridge/mockBridge";
import type { ParameterSnapshot, TelemetryDescriptor } from "../../domain/types";
import { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import type { ResolvedControlLoop } from "../../vehicleProfiles/types";
import { TelemetryStrip } from "./TelemetryStrip";

const descriptors: TelemetryDescriptor[] = [
  { channelId: 205, machineName: "drive.speed", displayName: "车速", group: "速度", unit: "m/s", telemetryType: "f32" },
  { channelId: 206, machineName: "drive.error", displayName: "速度误差", group: "速度", unit: "m/s", telemetryType: "f32" },
];

const records: ParameterSnapshot[] = [{
  paramId: 4,
  machineName: "control.target_speed_mps",
  displayName: "目标速度",
  group: "速度环",
  unit: "m/s",
  ramValue: { kind: "f32", value: 1.2 },
  persistedValue: { kind: "f32", value: 1.2 },
  revision: 1,
  dirty: false,
  syncKnown: true,
  writeState: "idle",
  writable: true,
  dangerous: true,
  lastError: null,
}];

const loop: ResolvedControlLoop = {
  id: "speed",
  label: "速度环",
  targetParamId: 4,
  targetWritable: true,
  gainParamIds: [],
  telemetry: { target: null, feedback: 205, error: 206, outputs: [] },
  recommendedChannelIds: [205, 206],
};

it("shows target, feedback, error, subscription, drop, and latency from existing data", async () => {
  const buffer = new TelemetryRingBuffer(8, 100);
  buffer.append([
    { channelId: 205, sampleSequence: 1, timestampUs: 10, value: { kind: "f32", value: 1.17 } },
    { channelId: 206, sampleSequence: 1, timestampUs: 10, value: { kind: "f32", value: -0.03 } },
  ]);
  const base = await new MockBridge().getSnapshot();
  const snapshot = {
    ...base,
    activeSubscription: { channelIds: [205, 206], sampleRateHz: 500, subscriptionVersion: 2 },
    diagnostics: { ...base.diagnostics, sequenceGapSamples: 2, deviceDroppedSamples: 3, lastRttMs: 8.4 },
  };

  render(<TelemetryStrip buffer={buffer} descriptors={descriptors} loop={loop} records={records} snapshot={snapshot} />);

  expect(screen.getByText("1.200")).toBeInTheDocument();
  expect(screen.getByText("1.170")).toBeInTheDocument();
  expect(screen.getByText("-0.030")).toBeInTheDocument();
  expect(screen.getByText("500 Hz")).toBeInTheDocument();
  expect(screen.getByText("5")).toBeInTheDocument();
  expect(screen.getByText("8.4 ms")).toBeInTheDocument();
});

it("uses an em dash instead of inventing missing telemetry", () => {
  render(<TelemetryStrip buffer={new TelemetryRingBuffer(8, 100)} descriptors={[]} loop={undefined} records={[]} snapshot={null} />);
  expect(screen.getAllByText("—").length).toBeGreaterThan(0);
});
