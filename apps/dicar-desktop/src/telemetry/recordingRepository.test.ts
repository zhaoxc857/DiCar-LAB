import { IDBFactory } from "fake-indexeddb";

import {
  RECORDING_FORMAT,
  calculateRecordingStats,
  createRecordingChunk,
  type TelemetryRecordingDocument,
  type TelemetryRecordingMetadata,
} from "./recordings";
import { RecordingRepository } from "./recordingRepository";

const FIRST_ID = "5fd2817e-0bb8-4510-9478-2ec7f78c84a1";
const SECOND_ID = "e5d3d9f6-6450-4d5e-9ec3-f18c20c24d89";
const THIRD_ID = "a4c5b4cb-5c63-4975-92a6-e8a342387e79";

function recordingMetadata(id = FIRST_ID, createdAtMs = 1_000): TelemetryRecordingMetadata {
  return {
    schemaVersion: 1,
    id,
    status: "recording",
    name: `recording-${createdAtMs}`,
    note: "repository test",
    createdAtMs,
    completedAtMs: null,
    stopReason: null,
    deviceIdHex: "d1ca000000000001",
    firmwareVersion: [0, 2, 0],
    vehicleProfileId: "dicar-diff-drive",
    storageGeneration: 4,
    transportIdentity: { endpoint: { kind: "simulator", address: "127.0.0.1:9000" } },
    subscription: { channelIds: [200], sampleRateHz: 100, subscriptionVersion: 9 },
    channelDescriptors: [{
      channelId: 200,
      telemetryType: "f32",
      machineName: "drive.speed_mps",
      displayName: "车速",
      group: "速度环",
      unit: "m/s",
    }],
    parameterSnapshot: [{
      paramId: 1,
      machineName: "pid.kp",
      ramValue: { kind: "f32", value: 1.2 },
      revision: 7,
    }],
    snapshotRevision: 12,
    markers: [],
    stats: {
      batchCount: 0,
      pointCount: 0,
      droppedSamples: 0,
      firstTimestampUs: null,
      lastTimestampUs: null,
      chunkCount: 0,
      logicalBytes: 0,
    },
  };
}

function recordingDocument(id = FIRST_ID, createdAtMs = 1_000): TelemetryRecordingDocument {
  const metadata = recordingMetadata(id, createdAtMs);
  const chunk = createRecordingChunk(id, 0, [{
    subscriptionVersion: 9,
    firstSampleSequence: 10,
    droppedSamples: 2,
    points: [{
      channelId: 200,
      timestampUs: 1_000_000 + createdAtMs,
      sampleSequence: 10,
      value: { kind: "f32", value: 1.25 },
    }],
  }]);
  return {
    format: RECORDING_FORMAT,
    schemaVersion: 1,
    metadata: {
      ...metadata,
      status: "complete",
      completedAtMs: createdAtMs + 1_000,
      stopReason: "manual",
      stats: calculateRecordingStats([chunk]),
    },
    chunks: [chunk],
  };
}

function createRepository(options: ConstructorParameters<typeof RecordingRepository>[0] = {}) {
  return new RecordingRepository({
    indexedDb: new IDBFactory(),
    databaseName: `recording-test-${crypto.randomUUID()}`,
    ...options,
  });
}

it("persists raw chunks in order and seals metadata from incremental stats", async () => {
  const repository = createRepository();
  const metadata = recordingMetadata();
  const first = createRecordingChunk(metadata.id, 0, recordingDocument().chunks[0]!.batches);
  const second = createRecordingChunk(metadata.id, 1, [{
    ...first.batches[0]!,
    firstSampleSequence: 11,
    droppedSamples: 0,
    points: [{
      ...first.batches[0]!.points[0]!,
      timestampUs: 1_020_000,
      sampleSequence: 11,
    }],
  }]);

  await repository.open();
  await repository.createRecording(metadata);
  await repository.appendChunk(second);
  await repository.appendChunk(first);
  await repository.sealRecording(metadata.id, "manual", 2_000, ["lap"]);

  const document = await repository.getDocument(metadata.id);
  expect(document?.chunks.map(({ chunkIndex }) => chunkIndex)).toEqual([0, 1]);
  expect(document?.metadata).toMatchObject({
    status: "complete",
    stopReason: "manual",
    markers: ["lap"],
    stats: { batchCount: 2, pointCount: 2, droppedSamples: 2, chunkCount: 2 },
  });
  repository.close();
});

it("removes unfinished metadata and all chunks when the database is reopened", async () => {
  const indexedDb = new IDBFactory();
  const databaseName = `recording-test-${crypto.randomUUID()}`;
  const first = new RecordingRepository({ indexedDb, databaseName });
  const metadata = recordingMetadata();
  await first.open();
  await first.createRecording(metadata);
  await first.appendChunk(createRecordingChunk(metadata.id, 0, recordingDocument().chunks[0]!.batches));
  first.close();

  const reopened = new RecordingRepository({ indexedDb, databaseName });
  await reopened.open();
  expect(await reopened.listRecordings()).toEqual([]);
  expect(await reopened.getChunks(metadata.id)).toEqual([]);
  reopened.close();
});

it("deletes the entire active recording after any chunk write failure", async () => {
  let failAppend = true;
  const repository = createRepository({
    faultInjector(operation) {
      if (operation === "append" && failAppend) {
        failAppend = false;
        throw new Error("quota exhausted");
      }
    },
  });
  const metadata = recordingMetadata();
  await repository.open();
  await repository.createRecording(metadata);

  await expect(repository.appendChunk(createRecordingChunk(metadata.id, 0, recordingDocument().chunks[0]!.batches)))
    .rejects.toThrow(/quota exhausted/);
  expect(await repository.getMetadata(metadata.id)).toBeNull();
  expect(await repository.getChunks(metadata.id)).toEqual([]);
  repository.close();
});

it("imports atomically, validates before writing, and rekeys duplicate IDs", async () => {
  let failAfterMetadata = false;
  const repository = createRepository({
    faultInjector(operation) {
      if (operation === "importAfterMetadata" && failAfterMetadata) throw new Error("transaction failed");
    },
  });
  await repository.open();
  const document = recordingDocument();

  const first = await repository.importJson(JSON.stringify(document));
  const second = await repository.importJson(
    JSON.stringify(document),
    undefined,
    () => SECOND_ID,
  );
  expect(first.metadata.id).toBe(FIRST_ID);
  expect(second.metadata.id).toBe(SECOND_ID);
  expect((await repository.listRecordings()).map(({ id }) => id).sort()).toEqual([FIRST_ID, SECOND_ID].sort());

  const damaged = structuredClone(recordingDocument(THIRD_ID, 3_000));
  damaged.metadata.stats.pointCount += 1;
  await expect(repository.importJson(JSON.stringify(damaged))).rejects.toThrow(/统计/);
  expect(await repository.getMetadata(THIRD_ID)).toBeNull();

  failAfterMetadata = true;
  await expect(repository.importJson(JSON.stringify(recordingDocument(THIRD_ID, 3_000))))
    .rejects.toThrow(/transaction failed/);
  expect(await repository.getMetadata(THIRD_ID)).toBeNull();
  expect(await repository.getChunks(THIRD_ID)).toEqual([]);
  repository.close();
});

it("prunes the oldest complete recording while preserving protected records", async () => {
  const repository = createRepository({ maxCount: 2 });
  await repository.open();
  await repository.importJson(JSON.stringify(recordingDocument(FIRST_ID, 1_000)));
  const release = repository.protect(FIRST_ID);
  await repository.importJson(JSON.stringify(recordingDocument(SECOND_ID, 2_000)));
  await repository.importJson(JSON.stringify(recordingDocument(THIRD_ID, 3_000)));

  expect((await repository.listRecordings()).map(({ id }) => id)).toEqual([THIRD_ID, FIRST_ID]);
  expect(await repository.getMetadata(SECOND_ID)).toBeNull();
  release();
  repository.close();
});

it("enforces logical capacity and rolls back an oversized import", async () => {
  const document = recordingDocument();
  const repository = createRepository({ maxBytes: document.metadata.stats.logicalBytes - 1 });
  await repository.open();

  await expect(repository.importJson(JSON.stringify(document))).rejects.toThrow(/容量/);
  expect(await repository.listRecordings()).toEqual([]);
  repository.close();
});
