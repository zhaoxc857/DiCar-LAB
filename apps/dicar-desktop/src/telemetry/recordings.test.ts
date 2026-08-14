import type { AppSnapshot, UiTelemetryBatch } from "../domain/types";
import {
  MAX_RECORDING_DURATION_MS,
  buildRecordingCsvBlob,
  buildRecordingJsonBlob,
  calculateRecordingStats,
  completeRecordingMetadata,
  createRecordingChunk,
  createRecordingMetadata,
  parseRecordingJson,
  recordingFileName,
  recordingStartDenial,
  rekeyImportedRecording,
  type TelemetryRecordingDocument,
} from "./recordings";

function readBlob(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("blob read failed"));
    reader.onload = () => resolve(String(reader.result));
    reader.readAsText(blob);
  });
}

function readBlobBytes(blob: Blob): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("blob read failed"));
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.readAsArrayBuffer(blob);
  });
}

function readySnapshot(): AppSnapshot {
  return {
    revision: 12,
    phase: "ready",
    transportIdentity: { endpoint: { kind: "simulator", address: "127.0.0.1:9000" } },
    sessionId: 3,
    deviceIdHex: "d1ca000000000001",
    firmwareVersion: [0, 2, 0],
    parameters: [{
      paramId: 1,
      machineName: "pid.kp",
      displayName: "Kp",
      group: "速度环",
      unit: "",
      ramValue: { kind: "f32", value: 1.2 },
      persistedValue: { kind: "f32", value: 1.2 },
      revision: 7,
      dirty: false,
      syncKnown: true,
      writeState: "idle",
      writable: true,
      dangerous: false,
      lastError: null,
      numeric: { min: 0, max: 5, step: 0.01 },
    }],
    telemetryDescriptors: [
      { channelId: 200, telemetryType: "f32", machineName: "drive.speed_mps", displayName: "车速", group: "速度环", unit: "m/s" },
      { channelId: 207, telemetryType: "f32", machineName: "drive.target_speed_mps", displayName: "目标", group: "速度环", unit: "m/s" },
      { channelId: 999, telemetryType: "u32", machineName: "unused", displayName: "未订阅", group: "其他", unit: "" },
    ],
    dirtyCount: 0,
    storageGeneration: 4,
    accessProfile: { role: "owner", leaseActive: true, localDemoOnly: true },
    desiredSubscription: { channelIds: [200, 207], sampleRateHz: 100, subscriptionVersion: 9 },
    activeSubscription: { channelIds: [200, 207], sampleRateHz: 100, subscriptionVersion: 9 },
    linkBudget: { hardwareProfile: null, baudRate: null, maxChannels: 8, maxSampleRateHz: 500, reason: "test" },
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
    markers: ["before recording"],
  };
}

function batch(firstSequence: number, firstTimestampUs: number, droppedSamples = 0): UiTelemetryBatch {
  return {
    subscriptionVersion: 9,
    firstSampleSequence: firstSequence,
    droppedSamples,
    points: [
      { channelId: 200, timestampUs: firstTimestampUs, sampleSequence: firstSequence, value: { kind: "f32", value: 1.25 } },
      { channelId: 207, timestampUs: firstTimestampUs, sampleSequence: firstSequence, value: { kind: "f32", value: 2 } },
      { channelId: 200, timestampUs: firstTimestampUs + 10_000, sampleSequence: firstSequence + 1, value: { kind: "f32", value: 1.4 } },
      { channelId: 207, timestampUs: firstTimestampUs + 10_000, sampleSequence: firstSequence + 1, value: { kind: "f32", value: 2 } },
    ],
  };
}

function completeDocument(): TelemetryRecordingDocument {
  const recording = createRecordingMetadata({
    id: "5fd2817e-0bb8-4510-9478-2ec7f78c84a1",
    name: "速度阶跃",
    note: "baseline",
    snapshot: readySnapshot(),
    vehicleProfileId: "dicar-diff-drive",
    createdAtMs: 1_000,
  });
  const chunks = [
    createRecordingChunk(recording.id, 0, [batch(10, 1_000_000, 2)]),
    createRecordingChunk(recording.id, 1, [batch(12, 1_020_000)]),
  ];
  return {
    format: "dicar-telemetry-recording",
    schemaVersion: 1,
    metadata: completeRecordingMetadata(recording, chunks, "manual", 2_000, ["T+1000000 µs"]),
    chunks,
  };
}

it("validates start eligibility and freezes only the active subscription metadata", () => {
  const snapshot = readySnapshot();
  expect(recordingStartDenial(snapshot, "  ", "")).toMatch(/名称/);
  expect(recordingStartDenial(snapshot, "x".repeat(65), "")).toMatch(/64/);
  expect(recordingStartDenial(snapshot, "valid", "x".repeat(257))).toMatch(/256/);
  expect(recordingStartDenial({ ...snapshot, paused: true }, "valid", "")).toMatch(/暂停/);
  expect(recordingStartDenial({ ...snapshot, activeSubscription: null }, "valid", "")).toMatch(/订阅/);
  expect(recordingStartDenial(snapshot, " valid ", " note ")).toBeNull();

  const metadata = createRecordingMetadata({
    id: "5fd2817e-0bb8-4510-9478-2ec7f78c84a1",
    name: " valid ",
    note: " note ",
    snapshot,
    vehicleProfileId: "dicar-diff-drive",
    createdAtMs: 1_000,
  });
  expect(metadata).toMatchObject({
    status: "recording",
    name: "valid",
    note: "note",
    deviceIdHex: "d1ca000000000001",
    firmwareVersion: [0, 2, 0],
    vehicleProfileId: "dicar-diff-drive",
    snapshotRevision: 12,
    storageGeneration: 4,
    subscription: { channelIds: [200, 207], sampleRateHz: 100, subscriptionVersion: 9 },
    parameterSnapshot: [{ paramId: 1, machineName: "pid.kp", ramValue: { kind: "f32", value: 1.2 }, revision: 7 }],
  });
  expect(metadata.channelDescriptors.map(({ channelId }) => channelId)).toEqual([200, 207]);
  expect(metadata.stats).toEqual({
    batchCount: 0,
    pointCount: 0,
    droppedSamples: 0,
    firstTimestampUs: null,
    lastTimestampUs: null,
    chunkCount: 0,
    logicalBytes: 0,
  });
});

it("recomputes complete stats and enforces the five minute duration", () => {
  const document = completeDocument();
  expect(document.metadata).toMatchObject({
    status: "complete",
    stopReason: "manual",
    completedAtMs: 2_000,
    markers: ["T+1000000 µs"],
    stats: {
      batchCount: 2,
      pointCount: 8,
      droppedSamples: 2,
      firstTimestampUs: 1_000_000,
      lastTimestampUs: 1_030_000,
      chunkCount: 2,
    },
  });
  expect(() => completeRecordingMetadata(
    { ...document.metadata, status: "recording", completedAtMs: null, stopReason: null },
    document.chunks,
    "durationLimit",
    document.metadata.createdAtMs + MAX_RECORDING_DURATION_MS + 1,
    [],
  )).toThrow(/5 分钟/);
});

it("round-trips schema v1 JSON and rekeys duplicate imports without overwrite", async () => {
  const document = completeDocument();
  const blob = buildRecordingJsonBlob(document);
  const parsed = parseRecordingJson(await readBlob(blob), blob.size);
  expect(parsed).toEqual(document);

  const rekeyed = rekeyImportedRecording(
    parsed,
    new Set([parsed.metadata.id]),
    () => "e5d3d9f6-6450-4d5e-9ec3-f18c20c24d89",
  );
  expect(rekeyed.metadata.id).toBe("e5d3d9f6-6450-4d5e-9ec3-f18c20c24d89");
  expect(rekeyed.chunks.every(({ recordingId }) => recordingId === rekeyed.metadata.id)).toBe(true);
});

it("rejects damaged or malicious JSON before it can be written", () => {
  const document = completeDocument();
  const damaged = structuredClone(document);
  damaged.chunks[0]!.batches[0]!.points[0]!.channelId = 999;
  expect(() => parseRecordingJson(JSON.stringify(damaged))).toThrow(/通道/);

  const badStats = structuredClone(document);
  badStats.metadata.stats.pointCount += 1;
  expect(() => parseRecordingJson(JSON.stringify(badStats))).toThrow(/统计/);

  const reversed = structuredClone(document);
  reversed.chunks[1]!.batches[0]!.points[0]!.timestampUs = 1;
  reversed.chunks[1] = createRecordingChunk(
    reversed.metadata.id,
    1,
    reversed.chunks[1]!.batches,
  );
  reversed.metadata.stats = calculateRecordingStats(reversed.chunks);
  expect(() => parseRecordingJson(JSON.stringify(reversed))).toThrow(/时间戳/);

  expect(() => parseRecordingJson("{}")) .toThrow(/格式/);
});

it("exports wide CSV with dropped-before placement, RFC 4180 quoting, and formula protection", async () => {
  const document = completeDocument();
  document.metadata.channelDescriptors[0]!.machineName = "=cmd,channel";
  const blob = buildRecordingCsvBlob(document);
  const csv = await readBlob(blob);
  const lines = csv.split("\r\n");

  expect([...await readBlobBytes(blob).then((bytes) => bytes.slice(0, 3))]).toEqual([0xef, 0xbb, 0xbf]);
  expect(lines[0]).toContain("\"'=cmd,channel\"");
  expect(lines[1]).toMatch(/^0,9,2,1000000,10,/);
  expect(lines[2]).toMatch(/^0,9,,1010000,11,/);
  expect(lines[3]).toMatch(/^1,9,0,1020000,12,/);
  expect(recordingFileName("../../ =危险 名称", "json")).toBe("dicar-recording-危险-名称.json");
});
