import { CircleNotch, LinkBreak, PlugsConnected } from "@phosphor-icons/react";
import { useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import { endpointLabel, type Endpoint, type SerialPortDescriptor } from "../../domain/types";
import { connectionLabel, useConnectionStore } from "../../stores/connectionStore";
import { Alert } from "../ui/alert";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Select } from "../ui/select";

type ConnectionMode = Endpoint["kind"];

export function ConnectionStatusBar() {
  const bridge = useDesktopBridge();
  const snapshot = useConnectionStore((state) => state.snapshot);
  const hydrated = useConnectionStore((state) => state.hydrated);
  const eventError = useConnectionStore((state) => state.eventError);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<ConnectionMode>("simulator");
  const [serialPorts, setSerialPorts] = useState<SerialPortDescriptor[]>([]);
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState(921_600);
  const ready = snapshot?.phase === "ready";

  async function selectMode(next: ConnectionMode) {
    setMode(next);
    setError(null);
    if (next !== "serial") return;
    try {
      const ports = await bridge.listSerialPorts();
      setSerialPorts(ports);
      setSelectedPort((current) => current || ports[0]?.portName || "");
      if (ports.length === 0) setError("未发现可用串口，请检查无线调试器 USB 连接");
    } catch (reason) {
      setSerialPorts([]);
      setSelectedPort("");
      setError(errorMessage(reason));
    }
  }

  async function toggleConnection() {
    setBusy(true);
    setError(null);
    try {
      const endpoint: Endpoint = mode === "simulator"
        ? { kind: "simulator", address: "127.0.0.1:7100" }
        : { kind: "serial", portName: selectedPort, baudRate };
      const result = ready ? await bridge.disconnect() : await bridge.connect(endpoint);
      if (result.status === "failed") setError(result.message);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  async function authorizeBrowserPort() {
    if (bridge.requestSerialPort === undefined) return;
    setBusy(true);
    setError(null);
    try {
      const port = await bridge.requestSerialPort();
      setSerialPorts((current) => [
        ...current.filter(({ portName }) => portName !== port.portName),
        port,
      ]);
      setSelectedPort(port.portName);
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
    }
  }

  const pendingEndpoint = mode === "simulator"
    ? "内置模拟器 · 等待连接"
    : selectedPort ? `${selectedPort} @ ${baudRate}` : "等待选择 COM";

  return (
    <>
      <section aria-label="连接状态" className="flex flex-wrap items-center justify-between gap-3 border-b border-(--border) bg-(--surface) px-4 py-2.5 lg:px-6">
        <div className="flex min-w-0 items-center gap-3">
          <span className={ready ? "text-(--success)" : "text-(--warning)"}>
            {busy ? <CircleNotch className="animate-spin" aria-hidden="true" size={20} /> : ready ? <PlugsConnected aria-hidden="true" size={20} /> : <LinkBreak aria-hidden="true" size={20} />}
          </span>
          <div className="min-w-44">
            <output aria-live="polite" className="block text-sm font-semibold">{hydrated ? connectionLabel(snapshot) : "载入状态"}</output>
            <p className="m-0 mt-0.5 font-mono text-[11px] text-(--text-muted)">{snapshot?.transportIdentity ? endpointLabel(snapshot.transportIdentity.endpoint) : pendingEndpoint}</p>
          </div>
          <Badge className="hidden sm:inline-flex">本地演示权限</Badge>
          <Badge className="hidden sm:inline-flex">单一活动主机</Badge>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Select aria-label="连接方式" className="h-8 w-32 text-xs" disabled={ready || busy} onChange={(event) => void selectMode(event.currentTarget.value as ConnectionMode)} value={mode}>
            <option value="simulator">模拟器体验</option>
            <option value="serial">真实串口</option>
          </Select>
          {mode === "serial" && (
            <>
              <Select aria-label="选择串口" className="h-8 w-40 font-mono text-xs" disabled={ready || busy || serialPorts.length === 0} onChange={(event) => setSelectedPort(event.currentTarget.value)} value={selectedPort}>
                <option value="">选择 COM</option>
                {serialPorts.map((port) => <option key={port.portName} value={port.portName}>{port.portName} · {port.displayName}</option>)}
              </Select>
              <Select aria-label="串口波特率" className="h-8 w-28 font-mono text-xs" disabled={ready || busy} onChange={(event) => setBaudRate(Number(event.currentTarget.value))} value={baudRate}>
                {[921600, 460800, 115200].map((rate) => <option key={rate} value={rate}>{rate}</option>)}
              </Select>
              {bridge.serialAccessMode === "browser" && (
                <Button disabled={ready || busy} onClick={() => void authorizeBrowserPort()} size="sm" variant="secondary">
                  授权浏览器串口
                </Button>
              )}
            </>
          )}
          <Button disabled={busy || (!ready && mode === "serial" && selectedPort === "")} onClick={() => void toggleConnection()} size="sm" variant={ready ? "secondary" : "primary"}>
            {ready ? "断开设备" : mode === "serial" ? "连接真实设备" : "连接模拟器"}
          </Button>
        </div>
      </section>
      {(error ?? eventError) && <div className="px-4 pt-3 lg:px-6"><Alert>{error ?? eventError}</Alert></div>}
    </>
  );
}

function errorMessage(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === "object" && reason !== null && "message" in reason && typeof reason.message === "string") return reason.message;
  return "连接操作失败";
}
