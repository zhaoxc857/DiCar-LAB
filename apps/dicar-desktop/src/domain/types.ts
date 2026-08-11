export type Endpoint =
  | { kind: "simulator"; address: string }
  | {
      kind: "serial";
      portName: string;
      baudRate: number;
      hardwareProfile: SerialHardwareProfile;
    };

export type SerialHardwareProfile = "nanoUartWl" | "hc05BluetoothSpp" | "genericSerial";
export type SerialPortKind = "usb" | "bluetooth" | "pci" | "unknown";

export interface SerialPortDescriptor {
  portName: string;
  displayName: string;
  vendorId: number | null;
  productId: number | null;
  portKind: SerialPortKind;
}

export function endpointLabel(endpoint: Endpoint | null): string {
  if (endpoint === null) return "尚未建立";
  return endpoint.kind === "simulator"
    ? `TCP ${endpoint.address}`
    : `${endpoint.portName} @ ${endpoint.baudRate}`;
}

export type TransportIdentity = {
  endpoint: Endpoint;
};

export type SnapshotPhase =
  | "disconnected"
  | "connecting"
  | "loadingManifest"
  | "loadingParameters"
  | "ready";

export type OperationStatus = "succeeded" | "failed" | "superseded" | "aborted";

export type ParameterValue =
  | { kind: "f32"; value: number }
  | { kind: "i32"; value: number }
  | { kind: "u32"; value: number }
  | { kind: "bool"; value: boolean }
  | { kind: "enum"; value: number };

export type TelemetryValue =
  | { kind: "f32"; value: number }
  | { kind: "i32"; value: number }
  | { kind: "u32"; value: number }
  | { kind: "flags32"; value: number };

export type AccessProfileId = "owner" | "tuner" | "observer";

export interface AccessProfile {
  role: AccessProfileId;
  leaseActive: boolean;
  localDemoOnly: true;
}

export interface OperationResult {
  operationId: number;
  status: OperationStatus;
  message: string;
}

export interface ParameterSnapshot {
  paramId: number;
  machineName: string;
  displayName: string;
  group: string;
  unit: string;
  ramValue: ParameterValue;
  persistedValue: ParameterValue | null;
  revision: number;
  dirty: boolean;
  syncKnown: boolean;
  writeState: "idle" | "inFlight" | "queued";
  writable: boolean;
  dangerous: boolean;
  lastError: string | null;
  description?: string;
  numeric?: { min: number; max: number; step: number };
  enumOptions?: Array<{ value: number; label: string }>;
}

export interface TelemetryDescriptor {
  channelId: number;
  telemetryType: "f32" | "i32" | "u32" | "flags32";
  machineName: string;
  displayName: string;
  group: string;
  unit: string;
}

export interface TelemetrySubscriptionRequest {
  channelIds: number[];
  sampleRateHz: number;
}

export interface TelemetrySubscriptionSnapshot extends TelemetrySubscriptionRequest {
  subscriptionVersion: number;
}

export interface TelemetryPoint {
  channelId: number;
  timestampUs: number;
  sampleSequence: number;
  value: TelemetryValue;
}

export interface UiTelemetryBatch {
  subscriptionVersion: number;
  firstSampleSequence: number;
  droppedSamples: number;
  points: TelemetryPoint[];
}

export interface DiagnosticsSnapshot {
  inboundBytes: number;
  outboundBytes: number;
  lastRttMs: number;
  lastValidFrameAtMs: number;
  validFrames: number;
  malformedFrames: number;
  crcErrors: number;
  decoderOverflows: number;
  retries: number;
  unsolicitedDropped: number;
  sequenceGapSamples: number;
  deviceDroppedSamples: number;
  rejectedTelemetryBatches: number;
  uiDroppedBatches: number;
}

export interface LinkBudgetSnapshot {
  hardwareProfile: SerialHardwareProfile | null;
  baudRate: number | null;
  maxChannels: number;
  maxSampleRateHz: number;
  reason: string;
}

export interface AppSnapshot {
  revision: number;
  phase: SnapshotPhase;
  transportIdentity: TransportIdentity | null;
  sessionId: number | null;
  deviceIdHex: string | null;
  firmwareVersion: [number, number, number] | null;
  parameters: ParameterSnapshot[];
  telemetryDescriptors: TelemetryDescriptor[];
  dirtyCount: number;
  storageGeneration: number;
  accessProfile: AccessProfile;
  desiredSubscription: TelemetrySubscriptionSnapshot | null;
  activeSubscription: TelemetrySubscriptionSnapshot | null;
  linkBudget: LinkBudgetSnapshot | null;
  paused: boolean;
  telemetryPoints: number;
  diagnostics: DiagnosticsSnapshot;
  lastDisconnectReason: string | null;
  markers: string[];
}

export type WindowCloseDecision =
  | "cancel"
  | "disconnectKeepUnknown"
  | "revertThenClose";

export type BridgeEvent =
  | { eventIndex: number; event: "snapshotChanged"; data: AppSnapshot }
  | { eventIndex: number; event: "telemetryBatch"; data: UiTelemetryBatch }
  | { eventIndex: number; event: "operationCompleted"; data: OperationResult }
  | { eventIndex: number; event: "connectionLost"; data: { message: string } }
  | {
      eventIndex: number;
      event: "fatalError";
      data: { code: string; message: string; operationId: number | null };
    }
  | {
      eventIndex: number;
      event: "windowCloseRequested";
      data: { requestId: number; dirtyCount: number; canRevert: boolean };
    };
