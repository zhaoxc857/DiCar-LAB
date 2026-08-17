export type FirmwareFlashPhase =
  | "preparing"
  | "switchingTransport"
  | "unlocking"
  | "erasing"
  | "programming"
  | "verifying"
  | "restarting"
  | "reconnecting"
  | "succeeded"
  | "recoveryRequired"
  | "retrying"
  | "rollingBack";

export type FirmwarePackageSummary = {
  releaseId: string;
  target: "lckfb-tmx-mspm0g3507";
  mcu: "MSPM0G3507";
  firmwareVersion: [number, number, number];
  imageLength: number;
  imageSha256: string;
  packageSha256: string;
  signingKeyId: string;
};

export type FirmwareFlashEvent = {
  operationId: string;
  phase: FirmwareFlashPhase;
  progressPercent: number;
  message: string;
};

export type FirmwareFlashResult = {
  operationId: string;
  deviceIdHex: string;
  firmwareVersion: [number, number, number];
  rolledBack: boolean;
};
