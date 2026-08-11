import type { Endpoint, OperationResult, SerialPortDescriptor } from "../domain/types";
import { MockBridge } from "./mockBridge";

export interface BrowserSerialPort {
  getInfo(): { usbVendorId?: number; usbProductId?: number };
  open(options: { baudRate: number }): Promise<void>;
  close(): Promise<void>;
}

export interface BrowserSerial {
  getPorts(): Promise<BrowserSerialPort[]>;
  requestPort(): Promise<BrowserSerialPort>;
}

export class WebSerialBridge extends MockBridge {
  readonly serialAccessMode = "browser" as const;
  readonly #serial: BrowserSerial;
  readonly #ports = new Map<string, BrowserSerialPort>();
  readonly #portNames = new Map<BrowserSerialPort, string>();
  #nextPortId = 1;

  constructor(serial: BrowserSerial) {
    super();
    this.#serial = serial;
  }

  override async listSerialPorts(): Promise<SerialPortDescriptor[]> {
    const ports = await this.#serial.getPorts();
    return ports.map((port) => this.#register(port));
  }

  async requestSerialPort(): Promise<SerialPortDescriptor> {
    return this.#register(await this.#serial.requestPort());
  }

  override async connect(endpoint: Endpoint): Promise<OperationResult> {
    if (endpoint.kind === "simulator") return super.connect(endpoint);
    const port = this.#ports.get(endpoint.portName);
    if (port === undefined) {
      return failed("浏览器串口授权已失效，请重新选择设备");
    }
    try {
      await port.open({ baudRate: endpoint.baudRate });
      await port.close();
    } catch (reason) {
      return failed(errorMessage(reason, "浏览器无法打开所选串口"));
    }
    return failed("Web Serial 端口已验证；DCTP 浏览器会话将在下一切片接入，当前不会伪装为已连接");
  }

  #register(port: BrowserSerialPort): SerialPortDescriptor {
    let portName = this.#portNames.get(port);
    if (portName === undefined) {
      portName = `WEB-SERIAL-${this.#nextPortId}`;
      this.#nextPortId += 1;
      this.#portNames.set(port, portName);
      this.#ports.set(portName, port);
    }
    const info = port.getInfo();
    const vendorId = info.usbVendorId ?? null;
    const productId = info.usbProductId ?? null;
    const usbLabel = vendorId === null || productId === null
      ? "设备"
      : `USB ${hex4(vendorId)}:${hex4(productId)}`;
    return {
      portName,
      displayName: `Web Serial ${usbLabel}`,
      vendorId,
      productId,
    };
  }
}

function failed(message: string): OperationResult {
  return { operationId: 0, status: "failed", message };
}

function errorMessage(reason: unknown, fallback: string): string {
  return reason instanceof Error ? `${fallback}：${reason.message}` : fallback;
}

function hex4(value: number): string {
  return value.toString(16).padStart(4, "0");
}
