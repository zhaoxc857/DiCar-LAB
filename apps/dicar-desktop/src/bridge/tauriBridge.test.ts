import { beforeEach, expect, it, vi } from "vitest";
import type { BridgeEvent, OperationResult } from "../domain/types";

const tauri = vi.hoisted(() => {
  const invoke = vi.fn();
  const channels: Array<{ onmessage?: (message: unknown) => void }> = [];
  class Channel<T> {
    onmessage?: (message: T) => void;

    constructor() {
      channels.push(this as { onmessage?: (message: unknown) => void });
    }
  }
  return { Channel, channels, invoke };
});

vi.mock("@tauri-apps/api/core", () => ({
  Channel: tauri.Channel,
  invoke: tauri.invoke,
}));

import { TauriBridge } from "./tauriBridge";

const success: OperationResult = {
  operationId: 9,
  status: "succeeded",
  message: "ok",
};

beforeEach(() => {
  tauri.channels.length = 0;
  tauri.invoke.mockReset();
  tauri.invoke.mockResolvedValue(success);
});

it("binds Channel.onmessage before opening and closes one subscription once", async () => {
  const expected: BridgeEvent = {
    eventIndex: 1,
    event: "connectionLost",
    data: { message: "test" },
  };
  tauri.invoke.mockImplementationOnce(async (command, args) => {
    expect(command).toBe("open_core_channel");
    const channel = (args as { onEvent: { onmessage?: (event: BridgeEvent) => void } }).onEvent;
    expect(channel.onmessage).toBeTypeOf("function");
    channel.onmessage?.(expected);
  });
  const received: BridgeEvent[] = [];
  const bridge = new TauriBridge();

  const unsubscribe = await bridge.subscribe((event) => received.push(event));
  unsubscribe();
  unsubscribe();

  expect(received).toEqual([expected]);
  expect(tauri.invoke).toHaveBeenNthCalledWith(
    1,
    "open_core_channel",
    expect.objectContaining({ onEvent: tauri.channels[0] }),
  );
  expect(tauri.invoke).toHaveBeenNthCalledWith(2, "close_core_channel");
  expect(tauri.invoke).toHaveBeenCalledTimes(2);
});

it("maps typed bridge calls to the exact Tauri commands and arguments", async () => {
  const bridge = new TauriBridge();
  const endpoint = { kind: "simulator" as const, address: "127.0.0.1:7100" };
  const value = { kind: "f32" as const, value: 2.5 };

  await bridge.connect(endpoint);
  tauri.invoke.mockResolvedValueOnce([
    { portName: "COM7", displayName: "无线 DAP", vendorId: 0x1a86, productId: 0x7523, portKind: "usb" },
  ]);
  const ports = await bridge.listSerialPorts();
  await bridge.writeParameter(1, value);
  await bridge.setTelemetrySubscription({ channelIds: [200, 201], sampleRateHz: 500 });
  await bridge.resolveWindowClose(7, "disconnectKeepUnknown");
  await bridge.getSnapshot();

  expect(tauri.invoke.mock.calls).toEqual([
    ["connect", { endpoint }],
    ["list_serial_ports"],
    ["write_parameter", { paramId: 1, value }],
    ["set_telemetry_subscription", { request: { channelIds: [200, 201], sampleRateHz: 500 } }],
    ["resolve_window_close", { requestId: 7, decision: "disconnectKeepUnknown" }],
    ["get_snapshot"],
  ]);
  expect(ports[0]).toMatchObject({ portName: "COM7", displayName: "无线 DAP" });
});
