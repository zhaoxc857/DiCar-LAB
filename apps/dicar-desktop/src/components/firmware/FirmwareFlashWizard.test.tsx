import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import type { FirmwareFlashPlatform } from "../../firmware/firmwarePlatform";
import type {
  FirmwareFlashEvent,
  FirmwareFlashResult,
  FirmwarePackageSummary,
} from "../../firmware/firmwareTypes";
import { FirmwareFlashWizard } from "./FirmwareFlashWizard";

const operationId = "123e4567-e89b-12d3-a456-426614174000";
const summary: FirmwarePackageSummary = {
  releaseId: operationId,
  target: "lckfb-tmx-mspm0g3507",
  mcu: "MSPM0G3507",
  firmwareVersion: [1, 2, 3],
  imageLength: 1024,
  imageSha256: "ab".repeat(32),
  packageSha256: "cd".repeat(32),
  signingKeyId: "0102030405060708",
};
const result: FirmwareFlashResult = {
  operationId,
  deviceIdHex: "19".repeat(16),
  firmwareVersion: [1, 2, 3],
  rolledBack: false,
};

class TestFirmwarePlatform implements FirmwareFlashPlatform {
  readonly available = true;
  inspect = vi.fn(async () => summary);
  start = vi.fn(async (
    _request: Parameters<FirmwareFlashPlatform["start"]>[0],
    listener: (event: FirmwareFlashEvent) => void,
  ) => {
    listener({ operationId, phase: "preparing", progressPercent: 15, message: "准备" });
    listener({ operationId, phase: "programming", progressPercent: 60, message: "写入" });
    listener({ operationId, phase: "succeeded", progressPercent: 100, message: "完成" });
    return result;
  });
  retry = vi.fn(async () => result);
  rollback = vi.fn(async () => ({ ...result, rolledBack: true }));
  cancel = vi.fn(async () => undefined);
}

function packageFile(): File {
  const file = new File([new Uint8Array([1, 2, 3])], "release.dicarfw");
  Object.defineProperty(file, "arrayBuffer", {
    value: async () => new Uint8Array([1, 2, 3]).buffer,
  });
  return file;
}

function renderWizard(platform: FirmwareFlashPlatform, currentVersion: [number, number, number] = [1, 0, 0]) {
  render(
    <AppProviders bridge={new MockBridge()} firmwarePlatform={platform}>
      <FirmwareFlashWizard
        currentVersion={currentVersion}
        onOpenChange={() => undefined}
        open
      />
    </AppProviders>,
  );
}

async function selectAndConfirmPackage() {
  fireEvent.change(screen.getByLabelText("选择 .dicarfw 固件包"), {
    target: { files: [packageFile()] },
  });
  expect(await screen.findByText("目标版本 1.2.3")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: /已停止车辆/ }));
}

it("inspects a package, requires confirmation and renders ordered success progress", async () => {
  const platform = new TestFirmwarePlatform();
  renderWizard(platform);

  await selectAndConfirmPackage();
  fireEvent.click(screen.getByRole("button", { name: "开始无线烧录" }));

  expect(await screen.findByText("固件升级完成")).toBeInTheDocument();
  expect(platform.start).toHaveBeenCalledWith(
    expect.objectContaining({
      packageBytes: new Uint8Array([1, 2, 3]),
      allowDowngrade: false,
    }),
    expect.any(Function),
  );
});

it("does not render a normal cancel action during erase or programming", async () => {
  const platform = new TestFirmwarePlatform();
  let release!: (value: FirmwareFlashResult) => void;
  platform.start.mockImplementation(async (_request, listener) => {
    listener({ operationId, phase: "erasing", progressPercent: 45, message: "正在擦除" });
    return new Promise((resolve) => { release = resolve; });
  });
  renderWizard(platform);

  await selectAndConfirmPackage();
  fireEvent.click(screen.getByRole("button", { name: "开始无线烧录" }));

  expect(await screen.findByText("正在擦除")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "取消" })).not.toBeInTheDocument();
  release(result);
  await screen.findByText("固件升级完成");
});

it("shows manual BSL recovery steps and can roll back the verified recovery package", async () => {
  const platform = new TestFirmwarePlatform();
  platform.start.mockImplementation(async (_request, listener) => {
    listener({
      operationId,
      phase: "recoveryRequired",
      progressPercent: 0,
      message: "需要人工恢复",
    });
    throw new Error("需要人工恢复");
  });
  renderWizard(platform);

  await selectAndConfirmPackage();
  fireEvent.click(screen.getByRole("button", { name: "开始无线烧录" }));

  expect(await screen.findByText(/按住 BSL/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "刷回恢复包" }));
  await waitFor(() => expect(platform.rollback).toHaveBeenCalledWith(operationId, expect.any(Function)));
});
