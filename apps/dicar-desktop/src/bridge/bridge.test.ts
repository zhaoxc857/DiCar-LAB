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

it("uses the requested 100 Hz period and produces telemetry while observed", async () => {
  vi.useFakeTimers();
  try {
    const bridge = new MockBridge();
    const batches: Extract<BridgeEvent, { event: "telemetryBatch" }>[] = [];
    const unsubscribe = await bridge.subscribe((event) => {
      if (event.event === "telemetryBatch") batches.push(event);
    });
    await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
    await bridge.setTelemetrySubscription({ channelIds: [207, 200], sampleRateHz: 100 });
    const before = batches.length;

    await vi.advanceTimersByTimeAsync(100);

    expect(batches.length).toBeGreaterThan(before);
    const points = batches.at(-1)!.data.points;
    expect(points.at(-2)!.timestampUs - points.at(-4)!.timestampUs).toBe(10_000);
    unsubscribe();
  } finally {
    vi.useRealTimers();
  }
});

it("keeps the dangerous target RAM-only and rejects invalid numeric writes", async () => {
  const bridge = new MockBridge();
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });

  for (const value of [Number.NaN, Number.POSITIVE_INFINITY, -0.1, 8.1]) {
    expect(await bridge.writeParameter(4, { kind: "f32", value })).toMatchObject({ status: "failed" });
  }
  expect(await bridge.writeParameter(4, { kind: "f32", value: 1 })).toMatchObject({ status: "succeeded" });

  const snapshot = await bridge.getSnapshot();
  expect(snapshot.dirtyCount).toBe(0);
  expect(snapshot.parameters.find(({ paramId }) => paramId === 4)).toMatchObject({
    ramValue: { kind: "f32", value: 1 },
    persistedValue: null,
    dangerous: true,
    dirty: false,
  });
});

it("drives target, speed, error, and PWM from RAM PID parameters", async () => {
  const bridge = new MockBridge();
  const batches: Extract<BridgeEvent, { event: "telemetryBatch" }>[] = [];
  const unsubscribe = await bridge.subscribe((event) => {
    if (event.event === "telemetryBatch") batches.push(event);
  });
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  await bridge.writeParameter(4, { kind: "f32", value: 1 });
  await bridge.setTelemetrySubscription({ channelIds: [207, 200, 208, 209], sampleRateHz: 100 });
  bridge.advanceTelemetry(300);
  unsubscribe();

  const points = batches.at(-1)!.data.points;
  const latest = (channelId: number) => points.filter((point) => point.channelId === channelId).at(-1)!.value.value;
  expect(latest(207)).toBe(1);
  expect(latest(200)).toBeGreaterThan(0.7);
  expect(Math.abs(latest(208))).toBeLessThan(0.3);
  expect(latest(209)).toBeLessThanOrEqual(1_000);
});

it("stops its real-time scheduler when paused, disconnected, or unobserved", async () => {
  vi.useFakeTimers();
  try {
    const bridge = new MockBridge();
    let batches = 0;
    const unsubscribe = await bridge.subscribe((event) => {
      if (event.event === "telemetryBatch") batches += 1;
    });
    await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
    await vi.advanceTimersByTimeAsync(40);
    expect(batches).toBeGreaterThan(1);

    await bridge.setPaused(true);
    const paused = batches;
    await vi.advanceTimersByTimeAsync(100);
    expect(batches).toBe(paused);

    await bridge.setPaused(false);
    await vi.advanceTimersByTimeAsync(40);
    expect(batches).toBeGreaterThan(paused);
    await bridge.disconnect();
    const disconnected = batches;
    await vi.advanceTimersByTimeAsync(100);
    expect(batches).toBe(disconnected);

    unsubscribe();
  } finally {
    vi.useRealTimers();
  }
});

it("clears desired and active subscriptions and stops real-time sampling", async () => {
  vi.useFakeTimers();
  try {
    const bridge = new MockBridge();
    let batches = 0;
    await bridge.subscribe((event) => {
      if (event.event === "telemetryBatch") batches += 1;
    });
    await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
    await bridge.clearTelemetrySubscription();

    expect(await bridge.getSnapshot()).toMatchObject({
      desiredSubscription: null,
      activeSubscription: null,
      paused: true,
    });
    const cleared = batches;
    await vi.advanceTimersByTimeAsync(100);
    expect(batches).toBe(cleared);
  } finally {
    vi.useRealTimers();
  }
});
