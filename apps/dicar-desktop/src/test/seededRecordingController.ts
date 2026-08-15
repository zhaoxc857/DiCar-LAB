import { IDBFactory } from "fake-indexeddb";
import { MockBridge } from "../bridge/mockBridge";
import { RecordingController } from "../stores/recordingStore";
import { RecordingRepository } from "../telemetry/recordingRepository";

export const SEEDED_RECORDING_IDS = [
  "5fd2817e-0bb8-4510-9478-2ec7f78c84a1",
  "e5d3d9f6-6450-4d5e-9ec3-f18c20c24d89",
  "a4c5b4cb-5c63-4975-92a6-e8a342387e79",
] as const;

export async function seededRecordingController() {
  let idIndex = 0;
  let now = 1_000;
  const repository = new RecordingRepository({
    indexedDb: new IDBFactory(),
    databaseName: `recording-ui-${crypto.randomUUID()}`,
  });
  const controller = new RecordingController(repository, {
    idFactory: () => SEEDED_RECORDING_IDS[idIndex++] as string,
    now: () => now,
  });
  const bridge = new MockBridge();
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  controller.setSnapshot(await bridge.getSnapshot());
  await controller.start({
    name: "较早记录",
    note: "first",
    vehicleProfileId: "generic-manifest",
  });
  await controller.stop("manual");
  now = 2_000;
  await controller.start({
    name: "最新记录",
    note: "second",
    vehicleProfileId: "generic-manifest",
  });
  await controller.stop("manual");
  return { bridge, controller };
}
