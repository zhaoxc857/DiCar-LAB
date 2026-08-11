import { WebSerialBridge, type BrowserSerial, type BrowserSerialPort } from "./webSerialBridge";

class FakePort implements BrowserSerialPort {
  openedWith: number | null = null;
  closeCount = 0;

  constructor(private readonly vendorId = 0x1a86, private readonly productId = 0x7523) {}

  getInfo() {
    return { usbVendorId: this.vendorId, usbProductId: this.productId };
  }

  async open(options: { baudRate: number }) {
    this.openedWith = options.baudRate;
  }

  async close() {
    this.closeCount += 1;
  }
}

class FakeSerial implements BrowserSerial {
  constructor(
    private readonly granted: BrowserSerialPort[],
    private readonly requested: BrowserSerialPort,
  ) {}

  async getPorts() {
    return this.granted;
  }

  async requestPort() {
    return this.requested;
  }
}

it("lists granted ports and registers a newly authorized browser port", async () => {
  const granted = new FakePort();
  const requested = new FakePort(0x303a, 0x1001);
  const bridge = new WebSerialBridge(new FakeSerial([granted], requested));

  expect(await bridge.listSerialPorts()).toEqual([
    {
      portName: "WEB-SERIAL-1",
      displayName: "Web Serial USB 1a86:7523",
      vendorId: 0x1a86,
      productId: 0x7523,
    },
  ]);
  expect(await bridge.requestSerialPort()).toMatchObject({
    portName: "WEB-SERIAL-2",
    vendorId: 0x303a,
    productId: 0x1001,
  });
});

it("opens the selected browser port but refuses to claim ready before DCTP is implemented", async () => {
  const port = new FakePort();
  const bridge = new WebSerialBridge(new FakeSerial([port], port));
  const [descriptor] = await bridge.listSerialPorts();

  const result = await bridge.connect({
    kind: "serial",
    portName: descriptor.portName,
    baudRate: 921600,
  });

  expect(port.openedWith).toBe(921600);
  expect(port.closeCount).toBe(1);
  expect(result).toMatchObject({
    status: "failed",
    message: expect.stringContaining("DCTP"),
  });
  expect((await bridge.getSnapshot()).phase).toBe("disconnected");
});

it("keeps the simulator experience available in a Web Serial browser", async () => {
  const port = new FakePort();
  const bridge = new WebSerialBridge(new FakeSerial([], port));

  const result = await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });

  expect(result.status).toBe("succeeded");
  expect((await bridge.getSnapshot()).phase).toBe("ready");
});
