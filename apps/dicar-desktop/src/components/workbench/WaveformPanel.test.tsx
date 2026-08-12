import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { WaveformPanel } from "./WaveformPanel";

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
  const descriptors = (await bridge.getSnapshot()).telemetryDescriptors;
  render(<AppProviders bridge={bridge}><WaveformPanel descriptors={descriptors} /></AppProviders>);
  await act(async () => { await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" }); });
  const canvas = screen.getByRole("img", { name: "实时波形" });
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, left: 0, top: 0, right: 400, bottom: 192, width: 400, height: 192, toJSON: () => ({}) });
  fireEvent.click(canvas, { clientX: 200 });

  await act(async () => { bridge.advanceTelemetry(30_000); });

  expect(await screen.findByText("游标数据已滚出缓冲，已移至最早样本")).toBeInTheDocument();
  expect(screen.getByRole("status", { name: "波形游标读数" })).toHaveTextContent(/游标 402000 µs/);
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
