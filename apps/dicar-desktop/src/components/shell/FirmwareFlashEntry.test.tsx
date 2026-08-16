import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";
import { FirmwareFlashEntry } from "./FirmwareFlashEntry";

it("keeps the reserved wireless flash action disabled while the backend is unavailable", () => {
  const onOpenFirmwareFlash = vi.fn();
  render(
    <FirmwareFlashEntry
      firmwareVersion={[0, 2, 0]}
      onOpenFirmwareFlash={onOpenFirmwareFlash}
      state={{ kind: "unavailable" }}
    />,
  );

  expect(screen.getByText("固件 0.2.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "无线烧录尚未启用" }));
  expect(onOpenFirmwareFlash).not.toHaveBeenCalled();
});

it.each([
  [{ kind: "checking" }, "正在检查设备"],
  [{ kind: "selecting" }, "选择固件文件"],
  [{ kind: "preparing" }, "正在准备烧录"],
  [{ kind: "flashing", progressPercent: 42 }, "烧录中 42%"],
  [{ kind: "succeeded" }, "烧录成功"],
  [{ kind: "failed", message: "连接中断" }, "烧录失败：连接中断"],
] as const)("labels the reserved wireless flash state %j", (state, label) => {
  render(<FirmwareFlashEntry firmwareVersion={null} state={state} />);
  expect(screen.getByText(label)).toBeInTheDocument();
  // 没有回调时按钮必须禁用，避免出现可点击但无动作的状态。
  expect(screen.getByRole("button", { name: "打开无线烧录" })).toBeDisabled();
});
