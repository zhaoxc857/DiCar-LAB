import { create } from "zustand";
import type { BridgeEvent, UiTelemetryBatch } from "../domain/types";

type WorkspaceState = {
  latestTelemetry: UiTelemetryBatch | null;
  receivedPoints: number;
  acceptEvent: (event: BridgeEvent) => void;
  reset: () => void;
};

export const useWorkspaceStore = create<WorkspaceState>((set) => ({
  latestTelemetry: null,
  receivedPoints: 0,
  acceptEvent: (event) => {
    if (event.event !== "telemetryBatch") return;
    set((state) => ({
      latestTelemetry: event.data,
      receivedPoints: state.receivedPoints + event.data.points.length,
    }));
  },
  reset: () => set({ latestTelemetry: null, receivedPoints: 0 }),
}));
