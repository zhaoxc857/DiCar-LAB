import { act, fireEvent, render, screen } from "@testing-library/react";
import { App } from "../../app/App";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import type { SerialPortDescriptor } from "../../domain/types";

class HardwareBridge extends MockBridge {
  override async listSerialPorts(): Promise<SerialPortDescriptor[]> {
    return [{
      portName: "COM12",
      displayName: "Bluetooth 串口",
      vendorId: null,
      productId: null,
      portKind: "bluetooth",
    }];
  }
}

it("opens all existing connection controls from the compact device chip", async () => {
  render(
    <AppProviders bridge={new HardwareBridge()}>
      <App />
    </AppProviders>,
  );
  await act(async () => undefined);

  fireEvent.click(screen.getByRole("button", { name: /未连接.*打开设备连接/ }));
  expect(screen.getByRole("dialog", { name: "设备连接" })).toBeInTheDocument();

  fireEvent.change(screen.getByRole("combobox", { name: "连接方式" }), {
    target: { value: "serial" },
  });
  expect(await screen.findByRole("option", { name: /COM12.*Bluetooth/ })).toBeInTheDocument();

  fireEvent.change(screen.getByRole("combobox", { name: "硬件类型" }), {
    target: { value: "hc05BluetoothSpp" },
  });
  fireEvent.click(screen.getByRole("button", { name: "硬件指南" }));

  expect(screen.getByText(/Windows 蓝牙设置中完成配对/)).toBeInTheDocument();
  expect(screen.getByText(/传出（Outgoing）COM/)).toBeInTheDocument();
  expect(screen.getByText(/3.3V 逻辑/)).toBeInTheDocument();
});
