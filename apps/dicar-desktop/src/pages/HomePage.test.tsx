import { act, fireEvent, render, screen } from "@testing-library/react";
import { App } from "../app/App";
import { AppProviders } from "../app/providers";
import { MockBridge } from "../bridge/mockBridge";
import { WebSerialBridge, type BrowserSerialPort } from "../bridge/webSerialBridge";

it("renders the B-style menu and connects the real simulator destination", async () => {
  const bridge = new MockBridge();
  render(
    <AppProviders bridge={bridge}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  expect(screen.getByRole("heading", { name: "工作区" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /实时调参与波形/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /计划发布.*数据记录与回放/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /计划发布.*参数方案库/ })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /连接与链路诊断/ })).toBeInTheDocument();
  expect(screen.getByText("本地演示权限")).toBeInTheDocument();
  expect(screen.getByText("未连接")).toBeInTheDocument();
  expect(screen.getByLabelText("连接方式")).toHaveValue("simulator");
  expect(screen.getByText("DCTP v1 · 模拟器待连接")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  expect(await screen.findByText("已就绪")).toBeInTheDocument();
  expect(screen.getByText("DCTP v1 · 模拟器已连接")).toBeInTheDocument();
  expect(screen.getByText("16 个遥测通道")).toBeInTheDocument();
  expect(screen.getByText("19 个参数")).toBeInTheDocument();
});

it("separates the real serial path and never reports a web preview as hardware", async () => {
  const bridge = new MockBridge();
  render(
    <AppProviders bridge={bridge}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  fireEvent.change(screen.getByLabelText("连接方式"), { target: { value: "serial" } });
  expect(await screen.findByText("当前 Web 预览不能访问真实串口，请使用桌面 App")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "连接真实设备" })).toBeDisabled();
  expect(screen.getByText("未连接")).toBeInTheDocument();

  const result = await bridge.connect({ kind: "serial", portName: "COM7", baudRate: 921600 });
  expect(result).toMatchObject({ status: "failed" });
  expect((await bridge.getSnapshot()).phase).toBe("disconnected");
});

it("authorizes a Web Serial port without claiming the DCTP device is ready", async () => {
  const port: BrowserSerialPort = {
    getInfo: () => ({ usbVendorId: 0x1a86, usbProductId: 0x7523 }),
    open: async () => undefined,
    close: async () => undefined,
  };
  const bridge = new WebSerialBridge({
    getPorts: async () => [],
    requestPort: async () => port,
  });
  render(
    <AppProviders bridge={bridge}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  fireEvent.change(screen.getByLabelText("连接方式"), { target: { value: "serial" } });
  fireEvent.click(await screen.findByRole("button", { name: "授权浏览器串口" }));
  expect(await screen.findByLabelText("选择串口")).toHaveValue("WEB-SERIAL-1");
  expect(screen.getByRole("button", { name: "连接真实设备" })).toBeEnabled();

  fireEvent.click(screen.getByRole("button", { name: "连接真实设备" }));
  expect(await screen.findByText(/DCTP 浏览器会话/)).toBeInTheDocument();
  expect(screen.getByText("未连接")).toBeInTheDocument();
});

it("keeps the skip link and exposes an honest deferred destination", async () => {
  render(
    <AppProviders bridge={new MockBridge()}>
      <App />
    </AppProviders>,
  );
  expect(screen.getByRole("link", { name: "跳至主要内容" })).toHaveAttribute(
    "href",
    "#main-content",
  );

  fireEvent.click(screen.getByRole("link", { name: /计划发布.*数据记录与回放/ }));
  expect(await screen.findByRole("heading", { name: "数据记录与回放" })).toBeInTheDocument();
  expect(screen.getByText(/首版后续阶段开放/)).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "返回工作区" })).toHaveAttribute("href", "/");
});
