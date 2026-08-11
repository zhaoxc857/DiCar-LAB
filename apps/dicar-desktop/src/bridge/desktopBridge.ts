import type {
  AccessProfileId,
  AppSnapshot,
  BridgeEvent,
  Endpoint,
  OperationResult,
  ParameterValue,
  SerialPortDescriptor,
  TelemetrySubscriptionRequest,
  WindowCloseDecision,
} from "../domain/types";

export interface DesktopBridge {
  readonly serialAccessMode?: "browser";
  listSerialPorts(): Promise<SerialPortDescriptor[]>;
  requestSerialPort?(): Promise<SerialPortDescriptor>;
  connect(endpoint: Endpoint): Promise<OperationResult>;
  disconnect(): Promise<OperationResult>;
  writeParameter(paramId: number, value: ParameterValue): Promise<OperationResult>;
  commitParameters(): Promise<OperationResult>;
  revertAll(): Promise<OperationResult>;
  undoLast(): Promise<OperationResult>;
  setTelemetrySubscription(request: TelemetrySubscriptionRequest): Promise<OperationResult>;
  setPaused(paused: boolean): Promise<OperationResult>;
  addMarker(label: string): Promise<OperationResult>;
  resolveWindowClose(
    requestId: number,
    decision: WindowCloseDecision,
  ): Promise<OperationResult>;
  selectAccessProfile(profile: AccessProfileId): Promise<OperationResult>;
  getSnapshot(): Promise<AppSnapshot>;
  subscribe(listener: (event: BridgeEvent) => void): Promise<() => void>;
}
