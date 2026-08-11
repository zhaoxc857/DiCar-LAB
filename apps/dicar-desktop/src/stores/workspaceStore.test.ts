import type { BridgeEvent, TelemetryPoint } from "../domain/types";
import { useWorkspaceStore } from "./workspaceStore";

function telemetryEvent(index: number, timestampUs: number): BridgeEvent {
  const points: TelemetryPoint[] = Array.from({ length: 8 }, (_, slot) => ({ channelId: 200 + slot, timestampUs, sampleSequence: index & 0xffff, value: { kind: "f32", value: index + slot } }));
  return { eventIndex: index + 1, event: "telemetryBatch", data: { subscriptionVersion: 1, firstSampleSequence: index & 0xffff, droppedSamples: 0, points } };
}

it("publishes at most thirty visual revisions per second while ingesting eight channels at 500 Hz", () => {
  const store = useWorkspaceStore.getState();
  store.reset();
  for (let index = 0; index < 500; index += 1) useWorkspaceStore.getState().acceptEvent(telemetryEvent(index, index * 2_000));
  const state = useWorkspaceStore.getState();
  expect(state.visualRevision).toBeGreaterThan(0);
  expect(state.visualRevision).toBeLessThanOrEqual(30);
  expect(state.buffer.totalPoints).toBe(4_000);
  expect(state.receivedPoints).toBe(4_000);
});

it("keeps no more than 8 x 30,000 retained points", () => {
  useWorkspaceStore.getState().reset();
  for (let batch = 0; batch < 301; batch += 1) {
    const points: TelemetryPoint[] = [];
    for (let sample = 0; sample < 100; sample += 1) {
      const sequence = batch * 100 + sample;
      for (let slot = 0; slot < 8; slot += 1) points.push({ channelId: 200 + slot, timestampUs: sequence * 2_000, sampleSequence: sequence & 0xffff, value: { kind: "u32", value: sequence } });
    }
    useWorkspaceStore.getState().acceptEvent({ eventIndex: batch + 1, event: "telemetryBatch", data: { subscriptionVersion: 1, firstSampleSequence: (batch * 100) & 0xffff, droppedSamples: 0, points } });
  }
  const state = useWorkspaceStore.getState();
  expect(state.buffer.totalPoints).toBe(8 * 30_000);
  for (let channelId = 200; channelId < 208; channelId += 1) expect(state.buffer.length(channelId)).toBe(30_000);
});
