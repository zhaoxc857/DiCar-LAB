import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { IDBFactory } from "fake-indexeddb";
import { vi } from "vitest";

import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import { RecordingController } from "../../stores/recordingStore";
import { RecordingRepository } from "../../telemetry/recordingRepository";
import { RecordingPlaybackDialog } from "./RecordingPlaybackDialog";

const RECORDING_ID = "5fd2817e-0bb8-4510-9478-2ec7f78c84a1";

async function playbackFixture() {
  const bridge = new MockBridge();
  await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
  const snapshot = await bridge.getSnapshot();
  const repository = new RecordingRepository({
    indexedDb: new IDBFactory(),
    databaseName: `playback-recording-${crypto.randomUUID()}`,
  });
  const controller = new RecordingController(repository, { idFactory: () => RECORDING_ID });
  controller.setSnapshot(snapshot);
  await controller.start({ name: "回放样本", note: "three frames", vehicleProfileId: "generic-manifest" });
  controller.acceptEvent({
    eventIndex: 1,
    event: "telemetryBatch",
    data: {
      subscriptionVersion: snapshot.activeSubscription!.subscriptionVersion,
      firstSampleSequence: 1,
      droppedSamples: 3,
      points: [1_000_000, 2_000_000, 3_000_000].map((timestampUs, index) => ({
        channelId: snapshot.activeSubscription!.channelIds[0]!,
        timestampUs,
        sampleSequence: index + 1,
        value: { kind: "f32" as const, value: index + 0.5 },
      })),
    },
  });
  await controller.drain();
  await controller.stop("manual");
  return { bridge, controller };
}

it("plays an independent timeline with seek, step, five speeds, and no device commands", async () => {
  const { bridge, controller } = await playbackFixture();
  const commandSpies = [
    vi.spyOn(bridge, "setTelemetrySubscription"),
    vi.spyOn(bridge, "setPaused"),
    vi.spyOn(bridge, "writeParameter"),
  ];
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.push(callback);
    return frames.length;
  });
  vi.stubGlobal("cancelAnimationFrame", () => undefined);
  try {
    render(
      <AppProviders bridge={bridge} recordingController={controller}>
        <RecordingPlaybackDialog onClose={() => undefined} open recordingId={RECORDING_ID} />
      </AppProviders>,
    );

    expect(await screen.findByRole("img", { name: "回放波形" })).toBeInTheDocument();
    const speed = screen.getByRole("combobox", { name: "回放速度" });
    expect(screen.getAllByRole("option", { name: /×/ }).map((option) => option.textContent)).toEqual(["0.25×", "0.5×", "1×", "2×", "4×"]);
    fireEvent.change(speed, { target: { value: "4" } });
    expect(speed).toHaveValue("4");

    fireEvent.change(screen.getByRole("slider", { name: "回放进度" }), { target: { value: "2000000" } });
    expect(screen.getByText("2.000 s / 3.000 s")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "下一采样时刻" }));
    expect(screen.getByText("3.000 s / 3.000 s")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "上一采样时刻" }));
    expect(screen.getByText("2.000 s / 3.000 s")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "播放回放" }));
    await act(async () => {
      const firstFrame = frames.splice(0);
      firstFrame.forEach((callback) => callback(1_000));
      await Promise.resolve();
      const secondFrame = frames.splice(0);
      secondFrame.forEach((callback) => callback(2_000));
    });
    expect(screen.getByText("3.000 s / 3.000 s")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "播放回放" })).toBeInTheDocument();
    commandSpies.forEach((spy) => expect(spy).not.toHaveBeenCalled());
  } finally {
    vi.unstubAllGlobals();
  }
});

it("protects the selected recording until the playback dialog closes", async () => {
  const { bridge, controller } = await playbackFixture();
  const { unmount } = render(
    <AppProviders bridge={bridge} recordingController={controller}>
      <RecordingPlaybackDialog onClose={() => undefined} open recordingId={RECORDING_ID} />
    </AppProviders>,
  );
  await screen.findByRole("dialog", { name: /回放 · 回放样本/ });
  await expect(controller.deleteRecording(RECORDING_ID)).rejects.toThrow(/回放或导出/);
  unmount();
  await waitFor(async () => expect(controller.deleteRecording(RECORDING_ID)).resolves.toBeUndefined());
});
