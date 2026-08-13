import type { AppSnapshot, ParameterSnapshot } from "../domain/types";
import {
  captureTuningSnapshot,
  diffTuningSnapshot,
  parseStoredSnapshot,
  snapshotExportJson,
  type TuningSnapshot,
} from "./snapshots";

function record(overrides: Partial<ParameterSnapshot>): ParameterSnapshot {
  return {
    paramId: 1,
    machineName: "pid.kp",
    displayName: "速度 Kp",
    group: "控制",
    unit: "",
    ramValue: { kind: "f32", value: 1 },
    persistedValue: { kind: "f32", value: 1 },
    revision: 0,
    dirty: false,
    syncKnown: true,
    writeState: "idle",
    writable: true,
    dangerous: false,
    lastError: null,
    numeric: { min: 0, max: 1000, step: 0.01 },
    ...overrides,
  };
}

function appSnapshot(parameters: ParameterSnapshot[]): AppSnapshot {
  return {
    revision: 3,
    phase: "ready",
    transportIdentity: null,
    sessionId: 77,
    deviceIdHex: "aa55",
    firmwareVersion: [1, 0, 0],
    parameters,
    telemetryDescriptors: [],
    dirtyCount: 0,
    storageGeneration: 4,
    accessProfile: { role: "owner", leaseActive: true, localDemoOnly: true },
    desiredSubscription: null,
    activeSubscription: null,
    linkBudget: null,
    paused: false,
    telemetryPoints: 0,
    diagnostics: {
      inboundBytes: 0,
      outboundBytes: 0,
      lastRttMs: 0,
      lastValidFrameAtMs: 0,
      validFrames: 0,
      malformedFrames: 0,
      crcErrors: 0,
      decoderOverflows: 0,
      retries: 0,
      unsolicitedDropped: 0,
      sequenceGapSamples: 0,
      deviceDroppedSamples: 0,
      rejectedTelemetryBatches: 0,
      uiDroppedBatches: 0,
    },
    lastDisconnectReason: null,
    markers: [],
  };
}

const meta = { name: "基准", note: "", origin: "manual" as const, profileId: "generic", nowMs: 1000, id: "id-1" };

describe("captureTuningSnapshot", () => {
  it("captures all RAM values and metadata", () => {
    const snapshot = captureTuningSnapshot(appSnapshot([record({}), record({ paramId: 2, machineName: "pid.ki" })]), meta);
    expect(snapshot).not.toBeNull();
    expect(snapshot?.entries).toHaveLength(2);
    expect(snapshot?.entries[0]).toMatchObject({ paramId: 1, machineName: "pid.kp", value: { kind: "f32", value: 1 } });
    expect(snapshot?.deviceIdHex).toBe("aa55");
    expect(snapshot?.storageGeneration).toBeNull();
  });

  it("records the storage generation only for commit records and rejects empty devices", () => {
    const committed = captureTuningSnapshot(appSnapshot([record({})]), { ...meta, origin: "commit" });
    expect(committed?.storageGeneration).toBe(4);
    expect(captureTuningSnapshot(appSnapshot([]), meta)).toBeNull();
  });
});

describe("diffTuningSnapshot", () => {
  const base = captureTuningSnapshot(appSnapshot([record({ ramValue: { kind: "f32", value: 2.5 } })]), meta) as TuningSnapshot;

  it("marks changed writable in-range entries as applicable", () => {
    const diff = diffTuningSnapshot(base, [record({})]);
    expect(diff.entries[0].disposition).toBe("apply");
    expect(diff.applicable).toHaveLength(1);
    expect(diff.blocked).toHaveLength(0);
  });

  it("skips identical values and blocks missing, retyped, out-of-range, read-only, and unknown entries", () => {
    expect(diffTuningSnapshot(base, [record({ ramValue: { kind: "f32", value: 2.5 } })]).entries[0].disposition).toBe("match");
    expect(diffTuningSnapshot(base, []).entries[0].disposition).toBe("missingParam");
    expect(diffTuningSnapshot(base, [record({ ramValue: { kind: "u32", value: 2 } })]).entries[0].disposition).toBe("typeChanged");
    expect(diffTuningSnapshot(base, [record({ numeric: { min: 0, max: 2, step: 0.1 } })]).entries[0].disposition).toBe("outOfRange");
    expect(diffTuningSnapshot(base, [record({ writable: false })]).entries[0].disposition).toBe("readOnly");
    expect(diffTuningSnapshot(base, [record({ syncKnown: false })]).entries[0].disposition).toBe("unknownState");
  });

  it("validates enum values against the current option list", () => {
    const enumBase = captureTuningSnapshot(
      appSnapshot([record({ paramId: 5, ramValue: { kind: "enum", value: 4 }, numeric: undefined, enumOptions: [{ value: 4, label: "4x" }] })]),
      meta,
    ) as TuningSnapshot;
    const shrunk = record({ paramId: 5, ramValue: { kind: "enum", value: 2 }, numeric: undefined, enumOptions: [{ value: 2, label: "2x" }] });
    expect(diffTuningSnapshot(enumBase, [shrunk]).entries[0].disposition).toBe("outOfRange");
    const compatible = record({ paramId: 5, ramValue: { kind: "enum", value: 2 }, numeric: undefined, enumOptions: [{ value: 2, label: "2x" }, { value: 4, label: "4x" }] });
    expect(diffTuningSnapshot(enumBase, [compatible]).entries[0].disposition).toBe("apply");
  });
});

describe("stored snapshot round trip", () => {
  it("survives JSON round trips and rejects corrupted entries", () => {
    const snapshot = captureTuningSnapshot(appSnapshot([record({})]), meta) as TuningSnapshot;
    const parsed = parseStoredSnapshot(JSON.parse(JSON.stringify(snapshot)));
    expect(parsed).toEqual(snapshot);
    expect(parseStoredSnapshot(null)).toBeNull();
    expect(parseStoredSnapshot({ ...snapshot, origin: "hacked" })).toBeNull();
    expect(parseStoredSnapshot({ ...snapshot, entries: [{ paramId: "x" }] })).toBeNull();
  });

  it("exports readable versioned JSON", () => {
    const snapshot = captureTuningSnapshot(appSnapshot([record({})]), meta) as TuningSnapshot;
    const exported = JSON.parse(snapshotExportJson(snapshot)) as { format: string; version: number; snapshot: TuningSnapshot };
    expect(exported.format).toBe("dicar-tuning-snapshot");
    expect(exported.version).toBe(1);
    expect(exported.snapshot.entries).toHaveLength(1);
  });
});
