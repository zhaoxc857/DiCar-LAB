import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";

import { AppProviders } from "../../app/providers";
import {
  SEEDED_RECORDING_IDS,
  seededRecordingController,
} from "../../test/seededRecordingController";
import { buildRecordingJsonBlob } from "../../telemetry/recordings";
import { RecordingManagerDialog } from "./RecordingManagerDialog";

function readBlob(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("blob read failed"));
    reader.onload = () => resolve(String(reader.result));
    reader.readAsText(blob);
  });
}

it("lists newest first, launches replay, exports both formats, and deletes", async () => {
  const { bridge, controller } = await seededRecordingController();
  const replay = vi.fn();
  const download = vi.fn();
  render(
    <AppProviders bridge={bridge} recordingController={controller}>
      <RecordingManagerDialog download={download} onClose={() => undefined} onReplay={replay} open />
    </AppProviders>,
  );

  const rows = await screen.findAllByTestId("recording-row");
  expect(rows.map((row) => row.textContent)).toEqual([
    expect.stringContaining("最新记录"),
    expect.stringContaining("较早记录"),
  ]);
  fireEvent.click(screen.getByRole("button", { name: "回放 最新记录" }));
  expect(replay).toHaveBeenCalledWith(SEEDED_RECORDING_IDS[1]);

  fireEvent.click(screen.getByRole("button", { name: "导出 JSON 最新记录" }));
  await waitFor(() => expect(download).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(screen.getByRole("button", { name: "导出 CSV 最新记录" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "导出 CSV 最新记录" }));
  await waitFor(() => expect(download).toHaveBeenCalledTimes(2));
  expect(download.mock.calls.map(([blob, fileName]) => [blob.type, fileName])).toEqual([
    ["application/json;charset=utf-8", expect.stringMatching(/\.json$/)],
    ["text/csv;charset=utf-8", expect.stringMatching(/\.csv$/)],
  ]);

  fireEvent.click(screen.getByRole("button", { name: "删除 最新记录" }));
  await waitFor(() => expect(screen.queryByText("最新记录")).not.toBeInTheDocument());
  expect(await controller.listRecordings()).toHaveLength(1);
});

it("imports a fully validated JSON document and reports damaged input without partial writes", async () => {
  const { bridge, controller } = await seededRecordingController();
  const existing = await controller.getDocument(SEEDED_RECORDING_IDS[0]);
  if (existing === null) throw new Error("seed recording missing");
  const validText = await readBlob(buildRecordingJsonBlob(existing));
  const damaged = structuredClone(existing);
  damaged.metadata.stats.pointCount += 1;
  render(
    <AppProviders bridge={bridge} recordingController={controller}>
      <RecordingManagerDialog onClose={() => undefined} onReplay={() => undefined} open />
    </AppProviders>,
  );
  const input = screen.getByLabelText("导入记录 JSON");

  await act(async () => {
    fireEvent.change(input, { target: { files: [new File([validText], "valid.json", { type: "application/json" })] } });
  });
  expect(await screen.findByText("记录导入成功")).toBeInTheDocument();
  expect(await controller.listRecordings()).toHaveLength(3);

  await act(async () => {
    fireEvent.change(input, { target: { files: [new File([JSON.stringify(damaged)], "damaged.json", { type: "application/json" })] } });
  });
  expect(await screen.findByText(/记录统计/)).toBeInTheDocument();
  expect(await controller.listRecordings()).toHaveLength(3);
});
