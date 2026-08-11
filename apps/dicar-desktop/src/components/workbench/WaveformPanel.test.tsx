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
