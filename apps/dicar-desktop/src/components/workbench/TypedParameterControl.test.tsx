import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import type { ParameterSnapshot, ParameterValue } from "../../domain/types";
import { TypedParameterControl } from "./TypedParameterControl";

function record(value: ParameterValue): ParameterSnapshot {
  return {
    paramId: 42,
    machineName: `fixture.${value.kind}`,
    displayName: `测试 ${value.kind}`,
    group: "测试",
    unit: value.kind === "f32" ? "m/s" : "",
    ramValue: value,
    persistedValue: value,
    revision: 7,
    dirty: false,
    syncKnown: true,
    writeState: "idle",
    writable: true,
    dangerous: false,
    lastError: null,
    numeric: value.kind === "f32" || value.kind === "i32" || value.kind === "u32"
      ? { min: 0, max: 100, step: value.kind === "f32" ? 0.1 : 1 }
      : undefined,
    enumOptions: value.kind === "enum"
      ? [{ value: 1, label: "模式一" }, { value: 2, label: "模式二" }]
      : undefined,
  };
}

it.each([
  [{ kind: "f32", value: 1 } as const, "2.5", { kind: "f32", value: 2.5 }],
  [{ kind: "i32", value: 1 } as const, "-2", { kind: "i32", value: -2 }],
  [{ kind: "u32", value: 1 } as const, "9", { kind: "u32", value: 9 }],
] as const)("submits a %s numeric parameter to RAM", async (initial, nextText, expected) => {
  const bridge = new MockBridge();
  const write = vi.spyOn(bridge, "writeParameter").mockResolvedValue({ operationId: 1, status: "succeeded", message: "ok" });
  render(<AppProviders bridge={bridge}><TypedParameterControl record={record(initial)} /></AppProviders>);
  await act(async () => undefined);

  fireEvent.change(screen.getByLabelText(`测试 ${initial.kind}`), { target: { value: nextText } });
  fireEvent.click(screen.getByRole("button", { name: "写入 RAM" }));

  await waitFor(() => expect(write).toHaveBeenCalledWith(42, expected));
});

it("submits a bool value without pretending it is a number", async () => {
  const bridge = new MockBridge();
  const write = vi.spyOn(bridge, "writeParameter").mockResolvedValue({ operationId: 1, status: "succeeded", message: "ok" });
  render(<AppProviders bridge={bridge}><TypedParameterControl record={record({ kind: "bool", value: false })} /></AppProviders>);
  await act(async () => undefined);
  fireEvent.click(screen.getByRole("switch", { name: "测试 bool" }));
  fireEvent.click(screen.getByRole("button", { name: "写入 RAM" }));
  await waitFor(() => expect(write).toHaveBeenLastCalledWith(42, { kind: "bool", value: true }));
});

it("submits an enum value without pretending it is a number", async () => {
  const bridge = new MockBridge();
  const write = vi.spyOn(bridge, "writeParameter").mockResolvedValue({ operationId: 1, status: "succeeded", message: "ok" });
  render(<AppProviders bridge={bridge}><TypedParameterControl record={record({ kind: "enum", value: 1 })} /></AppProviders>);
  await act(async () => undefined);
  fireEvent.change(screen.getByLabelText("测试 enum"), { target: { value: "2" } });
  fireEvent.click(screen.getByRole("button", { name: "写入 RAM" }));
  await waitFor(() => expect(write).toHaveBeenLastCalledWith(42, { kind: "enum", value: 2 }));
});

it("keeps readonly and dangerous semantics visible in text", async () => {
  const bridge = new MockBridge();
  const readonly = { ...record({ kind: "u32", value: 2048 }), writable: false, dangerous: true };
  render(<AppProviders bridge={bridge}><TypedParameterControl record={readonly} /></AppProviders>);
  await act(async () => undefined);
  expect(screen.getByLabelText("测试 u32")).toHaveAttribute("aria-readonly", "true");
  expect(screen.getByText("危险参数")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "写入 RAM" })).not.toBeInTheDocument();
});
