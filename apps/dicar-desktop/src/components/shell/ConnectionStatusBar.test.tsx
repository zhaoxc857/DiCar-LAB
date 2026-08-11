import { act, fireEvent, render, screen } from "@testing-library/react";
import { AppProviders } from "../../app/providers";
import { MockBridge } from "../../bridge/mockBridge";
import type { SerialPortDescriptor } from "../../domain/types";
import { ConnectionStatusBar } from "./ConnectionStatusBar";

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

it("shows HC-05 pairing, outgoing COM, and 3.3V guidance before connecting", async () => {
  render(
    <AppProviders bridge={new HardwareBridge()}>
      <ConnectionStatusBar />
    </AppProviders>,
  );
  await act(async () => undefined);

  fireEvent.change(screen.getByRole("combobox", { name: "连接方式" }), {
    target: { value: "serial" },
  });
  expect(await screen.findByRole("option", { name: /COM12.*Bluetooth/ })).toBeInTheDocument();

  fireEvent.change(screen.getByRole("combobox", { name: "硬件类型" }), {
    target: { value: "hc05BluetoothSpp" },
  });

  expect(screen.getByText(/Windows 蓝牙设置中完成配对/)).toBeInTheDocument();
  expect(screen.getByText(/传出（Outgoing）COM/)).toBeInTheDocument();
  expect(screen.getByText(/3.3V 逻辑/)).toBeInTheDocument();
  expect(screen.getByRole("option", { name: "自动探测" })).toBeInTheDocument();
});
