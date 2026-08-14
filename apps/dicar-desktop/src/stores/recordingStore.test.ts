import { IDBFactory } from "fake-indexeddb";
import type { AppSnapshot, BridgeEvent, UiTelemetryBatch } from "../domain/types";
import { RecordingRepository, type RecordingRepositoryOptions } from "../telemetry/recordingRepository";
import {
  MAX_RECORDING_DURATION_MS,
  RECORDING_CHUNK_POINT_LIMIT,
} from "../telemetry/recordings";
import { RecordingController, type RecordingControllerOptions } from "./recordingStore";

const RECORDING_ID = "5fd2817e-0bb8-4510-9478-2ec7f78c84a1";

function readySnapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return {
    revision: 12,
    phase: "ready",
    transportIdentity: { endpoint: { kind: "simulator", address: "127.0.0.1:9000" } },
    sessionId: 3,
    deviceIdHex: "d1ca000000000001",
    firmwareVersion: [0, 2, 0],
    parameters: [],
    telemetryDescriptors: [{
      channelId: 200,
      telemetryType: "f32",
      machineName: "drive.speed_mps",
      displayName: "车速",
      group: "速度环",
      unit: "m/s",
    }],
    dirtyCount: 0,
    storageGeneration: 4,
    accessProfile: { role: "owner", leaseActive: true, localDemoOnly: true },
    desiredSubscription: { channelIds: [200], sampleRateHz: 100, subscriptionVersion: 9 },
    activeSubscription: { channelIds: [200], sampleRateHz: 100, subscriptionVersion: 9 },
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
    markers: ["before"],
    ...overrides,
  };
}

function batch(timestampUs: number, pointCount = 1): UiTelemetryBatch {
  return {
    subscriptionVersion: 9,
    firstSampleSequence: 1,
    droppedSamples: 0,
    points: Array.from({ length: pointCount }, (_, index) => ({
      channelId: 200,
      timestampUs: timestampUs + index,
      sampleSequence: (index + 1) & 0xffff,
      value: { kind: "f32" as const, value: index / 10 },
    })),
  };
}

function telemetryEvent(data: UiTelemetryBatch, eventIndex = 1): BridgeEvent {
  return { eventIndex, event: "telemetryBatch", data };
}

function createController(
  repositoryOptions: RecordingRepositoryOptions = {},
  controllerOptions: RecordingControllerOptions = {},
) {
  const repository = new RecordingRepository({
    indexedDb: new IDBFactory(),
    databaseName: `controller-test-${crypto.randomUUID()}`,
    ...repositoryOptions,
  });
  const controller = new RecordingController(repository, {
    idFactory: () => RECORDING_ID,
    ...controllerOptions,
  });
  return { controller, repository };
}

async function start(controller: RecordingController, snapshot = readySnapshot()) {
  controller.setSnapshot(snapshot);
  await controller.start({ name: "test drive", note: "raw", vehicleProfileId: "dicar-diff-drive" });
}

it("flushes untouched batches at one second or 4096 points and preserves serial order", async () => {
  const { controller, repository } = createController();
  await start(controller);
  const first = batch(1_000_000);
  const second = batch(2_000_000);

  controller.acceptEvent(telemetryEvent(first));
  expect(await repository.getChunks(RECORDING_ID)).toEqual([]);
  controller.acceptEvent(telemetryEvent(second, 2));
  await controller.drain();
  expect((await repository.getChunks(RECORDING_ID))[0]?.batches).toEqual([first, second]);

  await controller.stop("manual");
  const complete = await repository.getDocument(RECORDING_ID);
  expect(complete?.metadata.stats).toMatchObject({ batchCount: 2, pointCount: 2, chunkCount: 1 });

  const large = createController();
  await start(large.controller);
  large.controller.acceptEvent(telemetryEvent(batch(10, RECORDING_CHUNK_POINT_LIMIT)));
  await large.controller.drain();
  expect(await large.repository.getChunks(RECORDING_ID)).toHaveLength(1);
});

it("automatically seals at exactly five minutes", async () => {
  let now = 10_000;
  const scheduler: { callback?: () => void } = {};
  const { controller, repository } = createController({}, {
    now: () => now,
    scheduleTimeout(callback) {
      scheduler.callback = callback;
      return 1 as unknown as ReturnType<typeof setTimeout>;
    },
    cancelTimeout: () => undefined,
  });
  await start(controller);
  now += MAX_RECORDING_DURATION_MS;
  const durationCallback = scheduler.callback;
  if (durationCallback === undefined) throw new Error("duration callback was not scheduled");
  durationCallback();
  await controller.drain();

  const metadata = await repository.getMetadata(RECORDING_ID);
  expect(metadata).toMatchObject({
    status: "complete",
    stopReason: "durationLimit",
    completedAtMs: 10_000 + MAX_RECORDING_DURATION_MS,
  });
  expect(controller.getState().active).toBeNull();
});

it.each([
  ["paused", readySnapshot({ paused: true, activeSubscription: null }), "paused"],
  ["disconnected", readySnapshot({ phase: "disconnected", activeSubscription: null }), "connectionLost"],
  ["subscription", readySnapshot({
    desiredSubscription: { channelIds: [200], sampleRateHz: 50, subscriptionVersion: 10 },
    activeSubscription: { channelIds: [200], sampleRateHz: 50, subscriptionVersion: 10 },
  }), "subscriptionChanged"],
] as const)("seals on %s snapshot transitions", async (_label, snapshot, expectedReason) => {
  const { controller, repository } = createController();
  await start(controller);
  controller.acceptEvent({ eventIndex: 2, event: "snapshotChanged", data: snapshot });
  await controller.drain();
  expect(await repository.getMetadata(RECORDING_ID)).toMatchObject({ status: "complete", stopReason: expectedReason });
});

it("stores only markers added during the recording", async () => {
  const { controller, repository } = createController();
  await start(controller);
  controller.acceptEvent({
    eventIndex: 2,
    event: "snapshotChanged",
    data: readySnapshot({ markers: ["before", "during"] }),
  });
  await controller.stop("manual");
  expect((await repository.getMetadata(RECORDING_ID))?.markers).toEqual(["during"]);
});

it("discards the complete active session and enters an error state after a write failure", async () => {
  const { controller, repository } = createController({
    faultInjector(operation) {
      if (operation === "append") throw new Error("disk full");
    },
  });
  await start(controller);
  controller.acceptEvent(telemetryEvent(batch(1, RECORDING_CHUNK_POINT_LIMIT)));
  await controller.drain();

  expect(await repository.getMetadata(RECORDING_ID)).toBeNull();
  expect(await repository.getChunks(RECORDING_ID)).toEqual([]);
  expect(controller.getState()).toMatchObject({ active: null, status: "error", error: "波形记录写入失败，已删除本次记录" });
});
