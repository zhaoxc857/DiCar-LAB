import {
  FIRMWARE_DESKTOP_ONLY_MESSAGE,
  TauriFirmwareFlashPlatform,
  UnavailableFirmwareFlashPlatform,
  type FirmwareInvoke,
} from "./firmwarePlatform";
import type { FirmwareFlashEvent } from "./firmwareTypes";

const operationId = "123e4567-e89b-12d3-a456-426614174000";

it("maps firmware inspection, start, retry, rollback and cancel to exact Tauri commands", async () => {
  let channelMessage: ((event: FirmwareFlashEvent) => void) | undefined;
  const channel = {
    set onmessage(listener: (event: FirmwareFlashEvent) => void) { channelMessage = listener; },
  };
  const invoke = vi.fn<FirmwareInvoke>(async (command) => {
    if (command === "firmware_inspect") return { firmwareVersion: [1, 2, 3] };
    return { operationId, deviceIdHex: "00".repeat(16), firmwareVersion: [1, 2, 3], rolledBack: false };
  });
  const platform = new TauriFirmwareFlashPlatform(invoke, () => channel);
  const listener = vi.fn();

  await platform.inspect(new Uint8Array([1, 2, 3]));
  await platform.start({ operationId, packageBytes: new Uint8Array([4, 5]), allowDowngrade: false }, listener);
  channelMessage?.({ operationId, phase: "preparing", progressPercent: 15, message: "准备" });
  await platform.retry(operationId, listener);
  await platform.rollback(operationId, listener);
  await platform.cancel(operationId);

  expect(invoke).toHaveBeenNthCalledWith(1, "firmware_inspect", { packageBytes: [1, 2, 3] });
  expect(invoke).toHaveBeenNthCalledWith(2, "firmware_start", {
    request: { operationId, packageBytes: [4, 5], allowDowngrade: false },
    onEvent: channel,
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "firmware_retry", { operationId, onEvent: channel });
  expect(invoke).toHaveBeenNthCalledWith(4, "firmware_rollback", { operationId, onEvent: channel });
  expect(invoke).toHaveBeenNthCalledWith(5, "firmware_cancel", { operationId });
  expect(listener).toHaveBeenCalledWith(expect.objectContaining({ phase: "preparing" }));
});

it("keeps firmware flashing explicitly unavailable outside Tauri", async () => {
  const platform = new UnavailableFirmwareFlashPlatform();
  expect(platform.available).toBe(false);
  await expect(platform.inspect(new Uint8Array())).rejects.toThrow(FIRMWARE_DESKTOP_ONLY_MESSAGE);
  await expect(platform.cancel(operationId)).rejects.toThrow(FIRMWARE_DESKTOP_ONLY_MESSAGE);
});
