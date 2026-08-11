import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import type { ParameterSnapshot, ParameterValue } from "../../domain/types";
import { EncoderCalibrationPanel } from "./EncoderCalibrationPanel";

function encoderRecord(paramId: number, machineName: string, displayName: string, value: ParameterValue): ParameterSnapshot {
  return { paramId, machineName, displayName, group: "编码器与车轮", unit: "", ramValue: value, persistedValue: value, revision: 1, dirty: false, syncKnown: true, writeState: "idle", writable: true, dangerous: false, lastError: null };
}

const encoderRecords = [
  encoderRecord(100, "encoder.left.ppr", "左编码器 PPR", { kind: "u32", value: 512 }),
  encoderRecord(101, "encoder.right.ppr", "右编码器 PPR", { kind: "u32", value: 512 }),
  encoderRecord(102, "encoder.quadrature_multiplier", "正交倍频", { kind: "enum", value: 4 }),
  encoderRecord(103, "encoder.left.inverted", "左侧反向", { kind: "bool", value: false }),
  encoderRecord(104, "encoder.right.inverted", "右侧反向", { kind: "bool", value: true }),
  encoderRecord(105, "vehicle.wheel_diameter_mm", "车轮直径", { kind: "f32", value: 64 }),
];

it("never conflates PPR, multiplier, and read-only CPR", async () => {
  const bridge = new MockBridge();
  const write = vi.spyOn(bridge, "writeParameter").mockResolvedValue({ operationId: 1, status: "succeeded", message: "ok" });
  render(<AppProviders bridge={bridge}><EncoderCalibrationPanel records={encoderRecords} /></AppProviders>);
  await act(async () => undefined);

  expect(screen.getByLabelText("左编码器 PPR")).toBeEnabled();
  expect(screen.getByLabelText("右编码器 PPR")).toBeEnabled();
  expect(screen.getByLabelText("正交倍频")).toHaveValue("4");
  expect(screen.getByLabelText("左有效 CPR")).toHaveAttribute("aria-readonly", "true");
  expect(screen.getByLabelText("右有效 CPR")).toHaveAttribute("aria-readonly", "true");
  expect(screen.queryByLabelText("编码器线数")).not.toBeInTheDocument();

  fireEvent.change(screen.getByLabelText("左编码器 PPR"), { target: { value: "600" } });
  expect(screen.getByLabelText("左有效 CPR")).toHaveValue("2400");
  expect(screen.getByLabelText("右有效 CPR")).toHaveValue("2048");
  fireEvent.change(screen.getByLabelText("正交倍频"), { target: { value: "2" } });
  expect(screen.getByLabelText("左有效 CPR")).toHaveValue("1200");
  expect(screen.getByLabelText("右有效 CPR")).toHaveValue("1024");
  fireEvent.click(screen.getByRole("button", { name: "应用编码器基准到 RAM" }));
  await waitFor(() => expect(write).toHaveBeenCalledWith(100, { kind: "u32", value: 600 }));
  expect(write).toHaveBeenCalledWith(101, { kind: "u32", value: 512 });
  expect(write).toHaveBeenCalledWith(102, { kind: "enum", value: 2 });
});

it("names every missing compatibility descriptor instead of inventing a substitute", async () => {
  render(<AppProviders bridge={new MockBridge()}><EncoderCalibrationPanel records={encoderRecords.filter(({ machineName }) => machineName !== "encoder.right.ppr")} /></AppProviders>);
  await act(async () => undefined);
  expect(screen.getByText(/缺少 encoder\.right\.ppr/)).toBeInTheDocument();
  expect(screen.queryByLabelText("右编码器 PPR")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("右有效 CPR")).not.toBeInTheDocument();
});
