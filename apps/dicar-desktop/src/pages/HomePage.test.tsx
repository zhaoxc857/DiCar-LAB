import { act, fireEvent, render, screen } from "@testing-library/react";
import { App } from "../app/App";
import { AppProviders } from "../app/providers";
import { MockBridge } from "../bridge/mockBridge";
import { WebSerialBridge, type BrowserSerialPort } from "../bridge/webSerialBridge";
import { seededRecordingController } from "../test/seededRecordingController";

beforeEach(() => {
  window.history.pushState({}, "", "/");
});

it("renders a truthful overview and connects the simulator from the device drawer", async () => {
  const bridge = new MockBridge();
  render(
    <AppProviders bridge={bridge}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  expect(screen.getByRole("heading", { name: "概览" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "进入实时调试" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "打开波形记录" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "查看诊断" })).toBeInTheDocument();
  expect(screen.queryByText("参数方案库")).not.toBeInTheDocument();
  expect(screen.getByText("通用 Manifest")).toBeInTheDocument();
  expect(screen.getByText("设备未连接")).toBeInTheDocument();

  openConnectionDrawer();
  expect(screen.getByText("本地演示权限")).toBeInTheDocument();
  expect(screen.getAllByText("未连接").length).toBeGreaterThanOrEqual(1);
  expect(screen.getByLabelText("连接方式")).toHaveValue("simulator");

  fireEvent.click(screen.getByRole("button", { name: "连接模拟器" }));
  expect((await screen.findAllByText("已就绪")).length).toBeGreaterThanOrEqual(1);
  expect(screen.getByText("16 个遥测通道")).toBeInTheDocument();
  expect(screen.getByText("19 个参数")).toBeInTheDocument();
});

it("shows the newest completed recordings from the existing recording controller", async () => {
  const { bridge, controller } = await seededRecordingController();
  render(
    <AppProviders bridge={bridge} recordingController={controller}>
      <App />
    </AppProviders>,
  );

  expect(await screen.findByText("最新记录")).toBeInTheDocument();
  expect(screen.getByText("较早记录")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "查看全部记录" })).toHaveAttribute("href", "/records");
});

it("separates the real serial path and never reports a web preview as hardware", async () => {
  const bridge = new MockBridge();
  render(
    <AppProviders bridge={bridge}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);
  openConnectionDrawer();

  fireEvent.change(screen.getByLabelText("连接方式"), { target: { value: "serial" } });
  expect(await screen.findByText("当前 Web 预览不能访问真实串口，请使用桌面 App")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "连接真实设备" })).toBeDisabled();
  expect(screen.getAllByText("未连接").length).toBeGreaterThanOrEqual(1);

  const result = await bridge.connect({ kind: "serial", portName: "COM7", baudRate: 921600, hardwareProfile: "genericSerial" });
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
  openConnectionDrawer();

  fireEvent.change(screen.getByLabelText("连接方式"), { target: { value: "serial" } });
  fireEvent.click(await screen.findByRole("button", { name: "授权浏览器串口" }));
  expect(await screen.findByLabelText("选择串口")).toHaveValue("WEB-SERIAL-1");
  expect(screen.getByRole("button", { name: "连接真实设备" })).toBeEnabled();

  fireEvent.click(screen.getByRole("button", { name: "连接真实设备" }));
  expect(await screen.findByText(/DCTP 浏览器会话/)).toBeInTheDocument();
  expect(screen.getAllByText("未连接").length).toBeGreaterThanOrEqual(1);
});

it("keeps the skip link and opens the completed recordings destination", async () => {
  render(
    <AppProviders bridge={new MockBridge()}>
      <App />
    </AppProviders>,
  );
  expect(screen.getByRole("link", { name: "跳至主要内容" })).toHaveAttribute(
    "href",
    "#main-content",
  );

  fireEvent.click(screen.getByRole("link", { name: "打开波形记录" }));
  expect(await screen.findByRole("heading", { name: "波形记录" })).toBeInTheDocument();
  expect(screen.queryByText(/首版后续阶段开放/)).not.toBeInTheDocument();
});

function openConnectionDrawer() {
  fireEvent.click(screen.getByRole("button", { name: /打开设备连接/ }));
  expect(screen.getByRole("dialog", { name: "设备连接" })).toBeInTheDocument();
}
