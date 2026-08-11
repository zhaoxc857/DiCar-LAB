import { create } from "zustand";
import type { AppSnapshot, BridgeEvent } from "../domain/types";

type ConnectionState = {
  snapshot: AppSnapshot | null;
  hydrated: boolean;
  lastEventIndex: number;
  eventError: string | null;
  setInitialSnapshot: (snapshot: AppSnapshot) => void;
  acceptEvent: (event: BridgeEvent) => void;
  reset: () => void;
};

const initialState = {
  snapshot: null,
  hydrated: false,
  lastEventIndex: 0,
  eventError: null,
};

export const useConnectionStore = create<ConnectionState>((set, get) => ({
  ...initialState,
  setInitialSnapshot: (snapshot) => set({ snapshot, hydrated: true }),
  acceptEvent: (event) => {
    const previous = get().lastEventIndex;
    if (event.eventIndex <= previous) return;
    const eventError = previous > 0 && event.eventIndex !== previous + 1
      ? `前端事件序号不连续：${previous} → ${event.eventIndex}`
      : get().eventError;
    set({
      lastEventIndex: event.eventIndex,
      eventError,
      ...(event.event === "snapshotChanged" ? { snapshot: event.data, hydrated: true } : {}),
    });
  },
  reset: () => set(initialState),
}));

export function connectionLabel(snapshot: AppSnapshot | null): string {
  switch (snapshot?.phase) {
    case "connecting": return "正在连接";
    case "loadingManifest": return "读取描述清单";
    case "loadingParameters": return "同步参数";
    case "ready": return "已就绪";
    default: return "未连接";
  }
}
