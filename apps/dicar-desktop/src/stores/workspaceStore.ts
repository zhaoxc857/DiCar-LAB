import { create } from "zustand";
import type { BridgeEvent, UiTelemetryBatch } from "../domain/types";
import { TelemetryRingBuffer } from "../telemetry/ringBuffer";

type WorkspaceState = {
  buffer: TelemetryRingBuffer;
  latestTelemetry: UiTelemetryBatch | null;
  receivedPoints: number;
  visualRevision: number;
  subscriptionVersion: number | null;
  lastVisualTimestampUs: number;
  acceptEvent: (event: BridgeEvent) => void;
  reset: () => void;
};

const VISUAL_INTERVAL_US = 1_000_000 / 30;

function initialWorkspaceState() {
  return { buffer: new TelemetryRingBuffer(8, 30_000), latestTelemetry: null, receivedPoints: 0, visualRevision: 0, subscriptionVersion: null, lastVisualTimestampUs: Number.NEGATIVE_INFINITY };
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  ...initialWorkspaceState(),
  acceptEvent: (event) => {
    if (event.event !== "telemetryBatch") return;
    const state = get();
    const changedSubscription = state.subscriptionVersion !== event.data.subscriptionVersion;
    if (changedSubscription) state.buffer.clear();
    state.buffer.append(event.data.points);
    const latestTimestampUs = event.data.points.at(-1)?.timestampUs ?? state.lastVisualTimestampUs;
    const shouldPublishVisual = changedSubscription || state.visualRevision === 0 || latestTimestampUs - state.lastVisualTimestampUs >= VISUAL_INTERVAL_US;
    set({
      buffer: state.buffer,
      latestTelemetry: event.data,
      receivedPoints: (changedSubscription ? 0 : state.receivedPoints) + event.data.points.length,
      subscriptionVersion: event.data.subscriptionVersion,
      visualRevision: state.visualRevision + (shouldPublishVisual ? 1 : 0),
      lastVisualTimestampUs: shouldPublishVisual ? latestTimestampUs : state.lastVisualTimestampUs,
    });
  },
  reset: () => set(initialWorkspaceState()),
}));
