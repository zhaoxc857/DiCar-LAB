import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { IDBFactory } from "fake-indexeddb";
import { vi } from "vitest";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { RecordingController } from "../../stores/recordingStore";
import { RecordingRepository } from "../../telemetry/recordingRepository";
import { WaveformPanel } from "./WaveformPanel";

function isolatedRecordingController(): RecordingController {
  return new RecordingController(new RecordingRepository({
    indexedDb: new IDBFactory(),
    databaseName: `waveform-recording-${crypto.randomUUID()}`,
  }));
}

it("enforces eight channels and sends the 500 Hz subscription", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  const setSubscription = vi.spyOn(bridge, "setTelemetrySubscription");
  render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);
  await act(async () => undefined);

  fireEvent.click(screen.getByRole("button", { name: "选择通道 8/8" }));
  expect(screen.getAllByRole("checkbox", { name: /模拟通道/ })).toHaveLength(16);
  fireEvent.click(screen.getByRole("checkbox", { name: "模拟通道 9" }));
  expect(screen.getByText("最多同时显示 8 个通道")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "应用 500 Hz 订阅" }));
  await waitFor(() => expect(setSubscription).toHaveBeenCalledWith({ channelIds: descriptors.slice(0, 8).map(({ channelId }) => channelId), sampleRateHz: 500 }));
});

it("reduces channel and sample-rate controls to the active HC-05 link budget", async () => {
  class Hc05Bridge extends MockBridge {
    override async getSnapshot() {
      return {
        ...await super.getSnapshot(),
        linkBudget: {
          hardwareProfile: "hc05BluetoothSpp" as const,
          baudRate: 115_200,
          maxChannels: 4,
          maxSampleRateHz: 50,
          reason: "HC-05 @ 115200 baud：最多 4 通道 × 50 Hz",
        },
      };
    }
  }
  const bridge = new Hc05Bridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);

  expect(await screen.findByRole("button", { name: "选择通道 4/4" })).toBeInTheDocument();
  expect(screen.getByText("HC-05 @ 115200 baud：最多 4 通道 × 50 Hz")).toBeInTheDocument();
  expect(screen.getByRole("combobox", { name: "遥测采样率" })).toHaveValue("50");
  expect(screen.queryByRole("option", { name: "100 Hz" })).not.toBeInTheDocument();
});

it("supports Pause, focused Space, marker, windows, and an accessible historical cursor", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  const pause = vi.spyOn(bridge, "setPaused");
  const marker = vi.spyOn(bridge, "addMarker");
  render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);
  await act(async () => undefined);
  await act(async () => { await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" }); bridge.advanceTelemetry(20); });

  fireEvent.keyDown(document.body, { key: " " });
  expect(pause).not.toHaveBeenCalled();
  const region = screen.getByRole("region", { name: "实时波形交互区" });
  region.focus();
  fireEvent.keyDown(region, { key: " " });
  await waitFor(() => expect(pause).toHaveBeenCalledWith(true));
  fireEvent.click(screen.getByRole("button", { name: "30 秒" }));
  expect(screen.getByRole("button", { name: "30 秒" })).toHaveAttribute("aria-pressed", "true");

  const summaryBefore = screen.getByRole("status", { name: "波形游标读数" }).textContent;
  fireEvent.keyDown(region, { key: "ArrowLeft" });
  const summaryAfter = screen.getByRole("status", { name: "波形游标读数" }).textContent;
  expect(summaryAfter).not.toBe(summaryBefore);
  expect(summaryAfter).toMatch(/µs/);
  expect(summaryAfter).toMatch(/模拟通道 1.*m\/s/);
  fireEvent.keyDown(region, { key: "m" });
  await waitFor(() => expect(marker).toHaveBeenCalledWith(expect.stringMatching(/^T\+\d+ µs$/)));
});

it("changes pending channels through a semantic workgroup only after Apply", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors.map((descriptor, index) => ({
    ...descriptor,
    machineName: index < 3 ? `motor.wheel_${index}_rpm` : `other.channel_${index}`,
    displayName: index < 3 ? `电机转速 ${index + 1}` : descriptor.displayName,
  }));
  const setSubscription = vi.spyOn(bridge, "setTelemetrySubscription");
  render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);

  fireEvent.change(screen.getByRole("combobox", { name: "波形工作组" }), { target: { value: "motor" } });
  expect(setSubscription).not.toHaveBeenCalled();
  expect(screen.getByText("3/8 通道")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "应用 500 Hz 订阅" }));
  await waitFor(() => expect(setSubscription).toHaveBeenCalledWith({ channelIds: [200, 201, 202], sampleRateHz: 500 }));

  fireEvent.click(screen.getByRole("button", { name: "选择通道 3/8" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "模拟通道 4" }));
  expect(screen.getByRole("combobox", { name: "波形工作组" })).toHaveValue("custom");
});

it("locks timestamp A and B on the canvas and retains them while paused", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);
  await act(async () => { await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" }); });
  const canvas = screen.getByRole("img", { name: "实时波形" });
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, left: 0, top: 0, right: 400, bottom: 192, width: 400, height: 192, toJSON: () => ({}) });

  fireEvent.click(canvas, { clientX: 100 });
  fireEvent.click(canvas, { clientX: 300 });
  expect(screen.getByRole("status", { name: "波形游标读数" })).toHaveTextContent(/A .*µs.*B .*µs.*Δt/s);

  fireEvent.click(screen.getByRole("button", { name: "暂停波形" }));
  await waitFor(() => expect(screen.getByRole("status", { name: "波形游标读数" })).toHaveTextContent(/Δt/));
  fireEvent.click(screen.getByRole("button", { name: "清除 A/B" }));
  expect(screen.getByRole("status", { name: "波形游标读数" })).not.toHaveTextContent(/Δt/);
});

it("exits fixed Y range when manual channel selection changes", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);
  await act(async () => undefined);

  fireEvent.change(screen.getByRole("combobox", { name: "Y 轴范围" }), { target: { value: "fixed" } });
  expect(screen.getByRole("combobox", { name: "Y 轴范围" })).toHaveValue("fixed");
  fireEvent.click(screen.getByRole("button", { name: "选择通道 8/8" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "模拟通道 8" }));
  expect(screen.getByRole("combobox", { name: "Y 轴范围" })).toHaveValue("local");
});

it("moves a locked cursor to the retained boundary when its samples roll out", async () => {
  const bridge = new MockBridge();
  const scheduler = vi.spyOn(globalThis, "setInterval").mockImplementation(() => 0 as unknown as ReturnType<typeof setInterval>);
  try {
    const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
    render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);
    await act(async () => { await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" }); });
    const canvas = screen.getByRole("img", { name: "实时波形" });
    vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, left: 0, top: 0, right: 400, bottom: 192, width: 400, height: 192, toJSON: () => ({}) });
    fireEvent.click(canvas, { clientX: 200 });

    await act(async () => { bridge.advanceTelemetry(30_000); });
    const retainedBoundaryUs = Math.min(...descriptors.slice(0, 8).flatMap(({ channelId }) => {
      const point = useWorkspaceStore.getState().buffer.first(channelId);
      return point === undefined ? [] : [point.timestampUs];
    }));

    expect(await screen.findByText("游标数据已滚出缓冲，已移至最早样本")).toBeInTheDocument();
    expect(Number.isFinite(retainedBoundaryUs)).toBe(true);
    expect(screen.getByRole("status", { name: "波形游标读数" })).toHaveTextContent(`游标 ${retainedBoundaryUs} µs`);
  } finally {
    await act(async () => { await bridge.disconnect(); });
    scheduler.mockRestore();
  }
});

it("applies each external request once and preserves later manual choices", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  const setSubscription = vi.spyOn(bridge, "setTelemetrySubscription");
  const { rerender } = render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} selectionRequest={{ requestId: 1, label: "速度环推荐", channelIds: [207, 200, 208, 209, 210] }} /></AppProviders>);
  await act(async () => undefined);
  expect(screen.getByText("5/8 通道")).toBeInTheDocument();
  expect(setSubscription).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "选择通道 5/8" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "模拟通道 11" }));
  expect(screen.getByText("4/8 通道")).toBeInTheDocument();
  rerender(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} selectionRequest={{ requestId: 1, label: "速度环推荐", channelIds: [207, 200, 208, 209, 210] }} /></AppProviders>);
  expect(screen.getByText("4/8 通道")).toBeInTheDocument();
  rerender(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} selectionRequest={{ requestId: 2, label: "速度环推荐", channelIds: [207, 200, 208] }} /></AppProviders>);
  expect(await screen.findByText("3/8 通道")).toBeInTheDocument();
  expect(setSubscription).not.toHaveBeenCalled();
});

it("clears stale pending channels when a refreshed recommendation has no Manifest matches", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  const { rerender } = render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} selectionRequest={{ requestId: 1, label: "速度环推荐", channelIds: [200] }} /></AppProviders>);
  expect(await screen.findByText("1/8 通道")).toBeInTheDocument();
  const changedManifest = descriptors.map((descriptor) => descriptor.channelId === 200 ? { ...descriptor, machineName: "reassigned.channel" } : descriptor);
  rerender(<AppProviders bridge={bridge}><WaveformPanel descriptors={changedManifest} selectionRequest={{ requestId: 2, label: "速度环推荐", channelIds: [] }} /></AppProviders>);
  expect(await screen.findByText("0/8 通道")).toBeInTheDocument();
  expect(screen.getByText("速度环推荐没有可用通道")).toBeInTheDocument();
});

it("starts and manually seals a named raw waveform recording from the toolbar", async () => {
  const bridge = new MockBridge();
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  const recordingController = isolatedRecordingController();
  render(
    <AppProviders bridge={bridge} recordingController={recordingController}>
      <WaveformPanel descriptors={descriptors} />
    </AppProviders>,
  );

  fireEvent.click(await screen.findByRole("button", { name: "开始波形记录" }));
  fireEvent.change(screen.getByLabelText("记录名称"), { target: { value: "速度阶跃" } });
  fireEvent.change(screen.getByLabelText("记录备注"), { target: { value: "100 Hz baseline" } });
  fireEvent.click(screen.getByRole("button", { name: "确认开始" }));
  expect(await screen.findByText(/正在记录 · 速度阶跃/)).toBeInTheDocument();

  await act(async () => { bridge.advanceTelemetry(10); });
  fireEvent.click(screen.getByRole("button", { name: "停止波形记录" }));
  expect(await screen.findByText("波形记录已保存")).toBeInTheDocument();
  expect((await recordingController.listRecordings())[0]).toMatchObject({
    name: "速度阶跃",
    note: "100 Hz baseline",
    status: "complete",
    stopReason: "manual",
  });
});

it("seals the active recording before applying a changed subscription", async () => {
  const bridge = new MockBridge();
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  const recordingController = isolatedRecordingController();
  recordingController.setSnapshot(await bridge.getSnapshot());
  await recordingController.start({ name: "before change", note: "", vehicleProfileId: "generic-manifest" });
  const stop = vi.spyOn(recordingController, "stop");
  const apply = vi.spyOn(bridge, "setTelemetrySubscription");
  render(
    <AppProviders bridge={bridge} recordingController={recordingController}>
      <WaveformPanel descriptors={descriptors} />
    </AppProviders>,
  );

  fireEvent.click(await screen.findByRole("button", { name: "应用 500 Hz 订阅" }));
  await waitFor(() => expect(apply).toHaveBeenCalled());
  expect(stop).toHaveBeenCalledWith("subscriptionChanged");
  expect(stop.mock.invocationCallOrder[0]).toBeLessThan(apply.mock.invocationCallOrder[0]!);
  expect((await recordingController.listRecordings())[0]?.stopReason).toBe("subscriptionChanged");
});

it("keeps the start form open with an explicit denial when the device is not ready", async () => {
  const bridge = new MockBridge();
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  render(
    <AppProviders bridge={bridge} recordingController={isolatedRecordingController()}>
      <WaveformPanel descriptors={descriptors} />
    </AppProviders>,
  );
  fireEvent.click(await screen.findByRole("button", { name: "开始波形记录" }));
  fireEvent.change(screen.getByLabelText("记录名称"), { target: { value: "not ready" } });
  fireEvent.click(screen.getByRole("button", { name: "确认开始" }));
  expect(await screen.findByText("设备就绪后才能开始录制")).toBeInTheDocument();
  expect(screen.getByRole("dialog", { name: "开始波形记录" })).toBeInTheDocument();
});
