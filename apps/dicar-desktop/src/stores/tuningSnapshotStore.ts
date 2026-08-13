import { create } from "zustand";
import { parseStoredSnapshot, type TuningSnapshot } from "../tuning/snapshots";

export const TUNING_SNAPSHOT_STORAGE_KEY = "dicar-tuning-snapshots";
export const MAX_SNAPSHOTS = 64;
const MAX_STORED_BYTES = 1024 * 1024;

export type SaveSnapshotResult = { status: "saved" } | { status: "failed"; message: string };

type TuningSnapshotState = {
  snapshots: TuningSnapshot[];
  issues: string[];
  saveSnapshot: (snapshot: TuningSnapshot) => SaveSnapshotResult;
  removeSnapshot: (id: string) => void;
  reset: () => void;
};

const initial = readPersistedSnapshots();

export const useTuningSnapshotStore = create<TuningSnapshotState>((set, get) => ({
  ...initial,
  saveSnapshot: (snapshot) => {
    const current = get().snapshots.filter(({ id }) => id !== snapshot.id);
    if (current.length >= MAX_SNAPSHOTS) {
      return { status: "failed", message: `最多保存 ${MAX_SNAPSHOTS} 个参数方案，请先删除旧方案` };
    }
    const next = [snapshot, ...current].sort((left, right) => right.createdAtMs - left.createdAtMs);
    if (storedBytes(next) > MAX_STORED_BYTES) {
      return { status: "failed", message: "参数方案总量超出 1 MiB，请先删除旧方案" };
    }
    const persistenceIssue = persist(next);
    if (persistenceIssue !== null) return { status: "failed", message: persistenceIssue };
    set({ snapshots: next, issues: [] });
    return { status: "saved" };
  },
  removeSnapshot: (id) => {
    const next = get().snapshots.filter((snapshot) => snapshot.id !== id);
    const persistenceIssue = persist(next);
    if (persistenceIssue !== null) {
      set({ issues: [persistenceIssue] });
      return;
    }
    set({ snapshots: next, issues: [] });
  },
  reset: () => {
    const persistenceIssue = persist([]);
    if (persistenceIssue !== null) {
      set({ issues: [persistenceIssue] });
      return;
    }
    set({ snapshots: [], issues: [] });
  },
}));

function readPersistedSnapshots(): Pick<TuningSnapshotState, "snapshots" | "issues"> {
  const fallback = { snapshots: [], issues: [] };
  if (typeof localStorage === "undefined") return fallback;
  try {
    const raw = localStorage.getItem(TUNING_SNAPSHOT_STORAGE_KEY);
    if (raw === null) return fallback;
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return { snapshots: [], issues: ["参数方案缓存格式无效，已清空"] };
    const snapshots: TuningSnapshot[] = [];
    const issues: string[] = [];
    for (const entry of parsed.slice(0, MAX_SNAPSHOTS)) {
      const snapshot = parseStoredSnapshot(entry);
      if (snapshot === null) {
        issues.push("已忽略一条损坏的参数方案");
        continue;
      }
      if (snapshots.some(({ id }) => id === snapshot.id)) continue;
      snapshots.push(snapshot);
    }
    snapshots.sort((left, right) => right.createdAtMs - left.createdAtMs);
    if (storedBytes(snapshots) > MAX_STORED_BYTES) {
      return { snapshots: [], issues: ["参数方案缓存超出 1 MiB 限制，已清空"] };
    }
    return { snapshots, issues };
  } catch {
    return { snapshots: [], issues: ["参数方案缓存损坏，已清空"] };
  }
}

function persist(snapshots: TuningSnapshot[]): string | null {
  if (typeof localStorage === "undefined") return null;
  try {
    localStorage.setItem(TUNING_SNAPSHOT_STORAGE_KEY, JSON.stringify(snapshots));
    return null;
  } catch {
    return "保存参数方案失败，已保留此前状态";
  }
}

function storedBytes(snapshots: TuningSnapshot[]): number {
  return new TextEncoder().encode(JSON.stringify(snapshots)).byteLength;
}
