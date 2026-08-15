import { act, fireEvent, render, screen } from "@testing-library/react";
import { App } from "../app/App";
import { AppProviders } from "../app/providers";
import { MockBridge } from "../bridge/mockBridge";

it("shows live snapshot identity and link diagnostics with text labels", async () => {
  window.history.pushState({}, "", "/diagnostics");
  const bridge = new MockBridge();
  render(
    <AppProviders bridge={bridge}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  expect(screen.getByRole("heading", { name: "连接与链路诊断" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /打开设备连接/ }));
  expect(screen.getByRole("dialog", { name: "设备连接" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  expect((await screen.findAllByText("TCP 127.0.0.1:7100")).length).toBeGreaterThanOrEqual(2);
  expect(screen.getByText("0x44aa0001")).toBeInTheDocument();
  expect(screen.getByText("接收字节")).toBeInTheDocument();
  expect(screen.getByText("CRC 错误")).toBeInTheDocument();
  expect(screen.getByText("设备丢样")).toBeInTheDocument();
  expect(screen.getByText("UI 丢批次")).toBeInTheDocument();
  expect(screen.getByText("遥测安全上限")).toBeInTheDocument();
  expect(screen.getByText("8 通道 × 500 Hz")).toBeInTheDocument();
});
