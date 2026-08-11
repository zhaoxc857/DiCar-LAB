import type { BridgeEvent } from "../domain/types";
import type { DesktopBridge } from "./desktopBridge";
import { MockBridge } from "./mockBridge";

it("delivers gap-free ordered snapshots and telemetry through one subscription", async () => {
  const bridge = new MockBridge();
  const events: BridgeEvent[] = [];
  const unsubscribe = await bridge.subscribe((event) => events.push(event));

  const result = await bridge.connect({
    kind: "simulator",
    address: "127.0.0.1:7100",
  });
  bridge.advanceTelemetry(40);
  unsubscribe();

  expect(result.status).toBe("succeeded");
  expect(events.map((event) => event.eventIndex)).toEqual(
    events.map((_, index) => index + 1),
  );
  expect(events.some((event) => event.event === "snapshotChanged")).toBe(true);
  expect(events.some((event) => event.event === "telemetryBatch")).toBe(true);
});

it("applies local demo permissions before RAM writes and Flash commits", async () => {
  const bridge = new MockBridge();
  const contract: DesktopBridge = bridge;
  await contract.connect({ kind: "simulator", address: "127.0.0.1:7100" });

  await contract.selectAccessProfile("observer");
  const deniedWrite = await contract.writeParameter(1, { kind: "f32", value: 2.5 });
  expect(deniedWrite).toMatchObject({
    status: "failed",
    message: "仅观察者不能修改参数",
  });
  expect((await contract.getSnapshot()).dirtyCount).toBe(0);

  await contract.selectAccessProfile("tuner");
  expect(await contract.writeParameter(1, { kind: "f32", value: 2.5 })).toMatchObject({
    status: "succeeded",
  });
  expect(await contract.commitParameters()).toMatchObject({
    status: "failed",
    message: "当前身份没有固化权限",
  });

  await contract.selectAccessProfile("owner");
  expect(await contract.commitParameters()).toMatchObject({ status: "succeeded" });
  const committed = await contract.getSnapshot();
  expect(committed.dirtyCount).toBe(0);
  expect(committed.storageGeneration).toBe(1);
  expect(committed.parameters.find(({ paramId }) => paramId === 1)).toMatchObject({
    ramValue: { kind: "f32", value: 2.5 },
    persistedValue: { kind: "f32", value: 2.5 },
  });
});

it("rejects invalid subscriptions and resumes with a new version", async () => {
  const bridge = new MockBridge();
  const contract: DesktopBridge = bridge;
  await contract.connect({ kind: "simulator", address: "127.0.0.1:7100" });

  expect(
    await contract.setTelemetrySubscription({
      channelIds: [200, 201, 202, 203, 204, 205, 206, 207, 208],
      sampleRateHz: 500,
    }),
  ).toMatchObject({ status: "failed" });
  expect(
    await contract.setTelemetrySubscription({
      channelIds: [200, 200],
      sampleRateHz: 500,
    }),
  ).toMatchObject({ status: "failed" });

  expect(
    await contract.setTelemetrySubscription({
      channelIds: [200, 201],
      sampleRateHz: 250,
    }),
  ).toMatchObject({ status: "succeeded" });
  const activeVersion = (await contract.getSnapshot()).activeSubscription?.subscriptionVersion;
  expect(activeVersion).toBe(2);
  await contract.setPaused(true);
  expect(await contract.getSnapshot()).toMatchObject({
    paused: true,
    activeSubscription: null,
    desiredSubscription: { subscriptionVersion: 2 },
  });
  await contract.setPaused(false);
  expect((await contract.getSnapshot()).activeSubscription?.subscriptionVersion).toBe(3);
});

it("serializes dirty window-close requests and rejects stale decisions", async () => {
  const bridge = new MockBridge();
  const events: BridgeEvent[] = [];
  await bridge.subscribe((event) => events.push(event));
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  await bridge.writeParameter(1, { kind: "f32", value: 3.25 });

  const requestId = bridge.requestWindowClose();
  expect(events.at(-1)).toMatchObject({
    event: "windowCloseRequested",
    data: { requestId, dirtyCount: 1, canRevert: true },
  });
  expect(await bridge.resolveWindowClose(requestId + 1, "cancel")).toMatchObject({
    status: "failed",
  });
  expect(await bridge.resolveWindowClose(requestId, "cancel")).toMatchObject({
    status: "succeeded",
  });
  expect((await bridge.getSnapshot()).phase).toBe("ready");
  expect(await bridge.resolveWindowClose(requestId, "disconnectKeepUnknown")).toMatchObject({
    status: "failed",
  });

  const disconnectRequest = bridge.requestWindowClose();
  expect(
    await bridge.resolveWindowClose(disconnectRequest, "disconnectKeepUnknown"),
  ).toMatchObject({ status: "succeeded" });
  const disconnected = await bridge.getSnapshot();
  expect(disconnected.phase).toBe("disconnected");
  expect(disconnected.parameters.every(({ syncKnown }) => !syncKnown)).toBe(true);
});
