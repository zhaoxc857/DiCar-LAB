import { Channel, invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  FirmwareFlashEvent,
  FirmwareFlashResult,
  FirmwarePackageSummary,
} from "./firmwareTypes";

export const FIRMWARE_DESKTOP_ONLY_MESSAGE = "无线固件烧录仅 Windows 桌面版可用";

export type FirmwareStartInput = {
  operationId: string;
  packageBytes: Uint8Array;
  allowDowngrade: boolean;
};

export interface FirmwareFlashPlatform {
  readonly available: boolean;
  inspect(packageBytes: Uint8Array): Promise<FirmwarePackageSummary>;
  start(
    request: FirmwareStartInput,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult>;
  retry(
    operationId: string,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult>;
  rollback(
    operationId: string,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult>;
  cancel(operationId: string): Promise<void>;
}

export type FirmwareInvoke = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

type FirmwareEventChannel = {
  onmessage: (event: FirmwareFlashEvent) => void;
};

type FirmwareEventChannelFactory = () => FirmwareEventChannel;

export class TauriFirmwareFlashPlatform implements FirmwareFlashPlatform {
  readonly available = true;

  constructor(
    private readonly invoke: FirmwareInvoke = tauriInvoke as FirmwareInvoke,
    private readonly createChannel: FirmwareEventChannelFactory = () => new Channel<FirmwareFlashEvent>(),
  ) {}

  inspect(packageBytes: Uint8Array): Promise<FirmwarePackageSummary> {
    return this.invoke("firmware_inspect", { packageBytes: [...packageBytes] })
      .then((value) => value as FirmwarePackageSummary)
      .catch(throwFirmwarePlatformError);
  }

  start(
    request: FirmwareStartInput,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult> {
    const onEvent = this.eventChannel(listener);
    return this.invoke("firmware_start", {
      request: {
        operationId: request.operationId,
        packageBytes: [...request.packageBytes],
        allowDowngrade: request.allowDowngrade,
      },
      onEvent,
    }).then((value) => value as FirmwareFlashResult).catch(throwFirmwarePlatformError);
  }

  retry(
    operationId: string,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult> {
    const onEvent = this.eventChannel(listener);
    return this.invoke("firmware_retry", { operationId, onEvent })
      .then((value) => value as FirmwareFlashResult)
      .catch(throwFirmwarePlatformError);
  }

  rollback(
    operationId: string,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult> {
    const onEvent = this.eventChannel(listener);
    return this.invoke("firmware_rollback", { operationId, onEvent })
      .then((value) => value as FirmwareFlashResult)
      .catch(throwFirmwarePlatformError);
  }

  cancel(operationId: string): Promise<void> {
    return this.invoke("firmware_cancel", { operationId })
      .then(() => undefined)
      .catch(throwFirmwarePlatformError);
  }

  private eventChannel(listener: (event: FirmwareFlashEvent) => void): FirmwareEventChannel {
    const channel = this.createChannel();
    channel.onmessage = listener;
    return channel;
  }
}

export class UnavailableFirmwareFlashPlatform implements FirmwareFlashPlatform {
  readonly available = false;

  async inspect(packageBytes: Uint8Array): Promise<FirmwarePackageSummary> {
    void packageBytes;
    throw new Error(FIRMWARE_DESKTOP_ONLY_MESSAGE);
  }

  async start(
    request: FirmwareStartInput,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult> {
    void request;
    void listener;
    throw new Error(FIRMWARE_DESKTOP_ONLY_MESSAGE);
  }

  async retry(
    operationId: string,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult> {
    void operationId;
    void listener;
    throw new Error(FIRMWARE_DESKTOP_ONLY_MESSAGE);
  }

  async rollback(
    operationId: string,
    listener: (event: FirmwareFlashEvent) => void,
  ): Promise<FirmwareFlashResult> {
    void operationId;
    void listener;
    throw new Error(FIRMWARE_DESKTOP_ONLY_MESSAGE);
  }

  async cancel(operationId: string): Promise<void> {
    void operationId;
    throw new Error(FIRMWARE_DESKTOP_ONLY_MESSAGE);
  }
}

function throwFirmwarePlatformError(reason: unknown): never {
  if (
    typeof reason === "object"
    && reason !== null
    && "message" in reason
    && typeof reason.message === "string"
  ) {
    throw new Error(reason.message);
  }
  throw new Error("无线固件烧录桌面通道调用失败");
}
