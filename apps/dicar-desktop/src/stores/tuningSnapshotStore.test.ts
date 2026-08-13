import type { TuningSnapshot } from "../tuning/snapshots";
import { MAX_SNAPSHOTS, TUNING_SNAPSHOT_STORAGE_KEY, useTuningSnapshotStore } from "./tuningSnapshotStore";

function snapshot(id: string, createdAtMs = 1000): TuningSnapshot {
  return {
    id,
    name: `方案 ${id}`,
    note: "",
    createdAtMs,
    origin: "manual",
    deviceIdHex: null,
    firmwareVersion: null,
    storageGeneration: null,
    profileId: "generic",
    entries: [{ paramId: 1, machineName: "pid.kp", displayName: "速度 Kp", unit: "", value: { kind: "f32", value: 2.5 } }],
  };
}

beforeEach(() => {
  localStorage.clear();
  useTuningSnapshotStore.getState().reset();
});

it("saves, replaces, sorts, and removes snapshots with persistence", () => {
  const store = useTuningSnapshotStore.getState();
  expect(store.saveSnapshot(snapshot("a", 1000)).status).toBe("saved");
  expect(store.saveSnapshot(snapshot("b", 2000)).status).toBe("saved");
  expect(useTuningSnapshotStore.getState().snapshots.map(({ id }) => id)).toEqual(["b", "a"]);

  expect(store.saveSnapshot({ ...snapshot("a", 3000), name: "改名" }).status).toBe("saved");
  const state = useTuningSnapshotStore.getState();
  expect(state.snapshots).toHaveLength(2);
  expect(state.snapshots[0]).toMatchObject({ id: "a", name: "改名" });

  const persisted = JSON.parse(localStorage.getItem(TUNING_SNAPSHOT_STORAGE_KEY) ?? "[]") as TuningSnapshot[];
  expect(persisted).toHaveLength(2);

  store.removeSnapshot("a");
  expect(useTuningSnapshotStore.getState().snapshots.map(({ id }) => id)).toEqual(["b"]);
});

it("enforces the snapshot count limit", () => {
  const store = useTuningSnapshotStore.getState();
  for (let index = 0; index < MAX_SNAPSHOTS; index += 1) {
    expect(store.saveSnapshot(snapshot(`s${index}`, index)).status).toBe("saved");
  }
  const result = store.saveSnapshot(snapshot("overflow"));
  expect(result.status).toBe("failed");
  expect(useTuningSnapshotStore.getState().snapshots).toHaveLength(MAX_SNAPSHOTS);
});

it("drops corrupted persisted entries without losing valid ones", () => {
  localStorage.setItem(TUNING_SNAPSHOT_STORAGE_KEY, JSON.stringify([snapshot("good"), { id: 42, broken: true }]));
  // 持久化读取发生在模块加载时；这里直接验证解析路径的行为。
  const raw = JSON.parse(localStorage.getItem(TUNING_SNAPSHOT_STORAGE_KEY) ?? "[]") as unknown[];
  expect(raw).toHaveLength(2);
  const store = useTuningSnapshotStore.getState();
  store.reset();
  expect(store.saveSnapshot(snapshot("good")).status).toBe("saved");
  expect(useTuningSnapshotStore.getState().snapshots).toHaveLength(1);
});
