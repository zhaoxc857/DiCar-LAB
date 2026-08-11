import { describe, expect, it, vi } from "vitest";
import { HARDWARE_PROFILES } from "./hardwareProfiles";
import { connectSerialWithProbe } from "./serialConnection";
import type { Endpoint, OperationResult } from "./types";

describe("serial hardware connection probing", () => {
  it("tries the HC-05 DCTP handshake rates in order and stops at the first ready device", async () => {
    const attempted: Endpoint[] = [];
    const results: OperationResult[] = [
      failed(1, "无握手响应"),
      failed(2, "无握手响应"),
      succeeded(3, "设备连接并加载完成"),
    ];
    const connect = vi.fn(async (endpoint: Endpoint) => {
      attempted.push(endpoint);
      return results.shift() ?? failed(4, "不应继续探测");
    });
    const onAttempt = vi.fn();

    const result = await connectSerialWithProbe(
      { connect },
      {
        hardwareProfile: "hc05BluetoothSpp",
        portName: "COM12",
        baudRate: "auto",
      },
      onAttempt,
    );

    expect(attempted).toEqual([
      serialEndpoint("COM12", 115_200, "hc05BluetoothSpp"),
      serialEndpoint("COM12", 9_600, "hc05BluetoothSpp"),
      serialEndpoint("COM12", 38_400, "hc05BluetoothSpp"),
    ]);
    expect(onAttempt).toHaveBeenCalledTimes(3);
    expect(result).toEqual({ operation: succeeded(3, "设备连接并加载完成"), baudRate: 38_400, attemptedBaudRates: [115_200, 9_600, 38_400] });
  });

  it("returns a disconnected failure after exhausting every profile rate", async () => {
    const connect = vi.fn(async (_endpoint: Endpoint) => failed(7, "未收到 DCTP 握手"));

    const result = await connectSerialWithProbe(
      { connect },
      { hardwareProfile: "nanoUartWl", portName: "COM8", baudRate: "auto" },
      () => undefined,
    );

    expect(connect).toHaveBeenCalledTimes(3);
    expect(result.baudRate).toBeNull();
    expect(result.attemptedBaudRates).toEqual([460_800, 230_400, 115_200]);
    expect(result.operation.status).toBe("failed");
  });

  it("uses one explicit baud rate without probing other values", async () => {
    const connect = vi.fn(async (_endpoint: Endpoint) => succeeded(9, "设备连接并加载完成"));

    const result = await connectSerialWithProbe(
      { connect },
      { hardwareProfile: "genericSerial", portName: "COM5", baudRate: 57_600 },
      () => undefined,
    );

    expect(connect).toHaveBeenCalledOnce();
    expect(result.baudRate).toBe(57_600);
    expect(HARDWARE_PROFILES.hc05BluetoothSpp.recommendedBaudRate).toBe(115_200);
  });
});

function serialEndpoint(
  portName: string,
  baudRate: number,
  hardwareProfile: "nanoUartWl" | "hc05BluetoothSpp" | "genericSerial",
): Endpoint {
  return { kind: "serial", portName, baudRate, hardwareProfile };
}

function failed(operationId: number, message: string): OperationResult {
  return { operationId, status: "failed", message };
}

function succeeded(operationId: number, message: string): OperationResult {
  return { operationId, status: "succeeded", message };
}
