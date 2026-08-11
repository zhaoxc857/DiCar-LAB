import { HARDWARE_PROFILES } from "./hardwareProfiles";
import type { Endpoint, OperationResult, SerialHardwareProfile } from "./types";

type SerialConnector = {
  connect(endpoint: Endpoint): Promise<OperationResult>;
};

export type SerialConnectionRequest = {
  hardwareProfile: SerialHardwareProfile;
  portName: string;
  baudRate: number | "auto";
};

export type SerialProbeResult = {
  operation: OperationResult;
  baudRate: number | null;
  attemptedBaudRates: number[];
};

export async function connectSerialWithProbe(
  connector: SerialConnector,
  request: SerialConnectionRequest,
  onAttempt: (baudRate: number) => void,
): Promise<SerialProbeResult> {
  const rates = request.baudRate === "auto"
    ? HARDWARE_PROFILES[request.hardwareProfile].probeBaudRates
    : [request.baudRate];
  const attemptedBaudRates: number[] = [];
  let operation: OperationResult = {
    operationId: 0,
    status: "failed",
    message: "没有可用的串口波特率",
  };

  for (const baudRate of rates) {
    attemptedBaudRates.push(baudRate);
    onAttempt(baudRate);
    operation = await connector.connect({
      kind: "serial",
      portName: request.portName,
      baudRate,
      hardwareProfile: request.hardwareProfile,
    });
    if (operation.status === "succeeded") {
      return { operation, baudRate, attemptedBaudRates };
    }
  }

  return { operation, baudRate: null, attemptedBaudRates };
}
