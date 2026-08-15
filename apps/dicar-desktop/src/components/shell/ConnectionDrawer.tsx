import { LinkBreak, PlugsConnected } from "@phosphor-icons/react";
import { useEffect, useState, type ReactNode } from "react";
import { useDesktopBridge } from "../../app/providers";
import { HARDWARE_PROFILES, SUPPORTED_SERIAL_BAUD_RATES } from "../../domain/hardwareProfiles";
import { connectSerialWithProbe } from "../../domain/serialConnection";
import { endpointLabel, type Endpoint, type SerialHardwareProfile, type SerialPortDescriptor } from "../../domain/types";
import { connectionLabel, useConnectionStore } from "../../stores/connectionStore";
import { useSettingsStore } from "../../stores/settingsStore";
import { Alert } from "../ui/alert";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Drawer } from "../ui/drawer";
import { Select } from "../ui/select";
import { FirmwareFlashEntry } from "./FirmwareFlashEntry";
import { HardwareConnectionGuide } from "./HardwareConnectionGuide";
import { VehicleSwitcher } from "./VehicleSwitcher";

type ConnectionMode = Endpoint["kind"];

export type ConnectionDrawerSection = "connection" | "guide" | "preferences";

type ConnectionDrawerProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialSection: ConnectionDrawerSection;
};

export function ConnectionDrawer({
  open,
  onOpenChange,
  initialSection,
}: ConnectionDrawerProps) {
  const bridge = useDesktopBridge();
  const snapshot = useConnectionStore((state) => state.snapshot);
  const hydrated = useConnectionStore((state) => state.hydrated);
  const eventError = useConnectionStore((state) => state.eventError);
  const savedProfile = useSettingsStore((state) => state.serialHardwareProfile);
  const savedPort = useSettingsStore((state) => state.serialPortName);
  const savedBaudRate = useSettingsStore((state) => state.serialBaudRate);
  const saveSerialConnection = useSettingsStore((state) => state.saveSerialConnection);
  const [section, setSection] = useState<ConnectionDrawerSection>(initialSection);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mode, setMode] = useState<ConnectionMode>("simulator");
  const [serialPorts, setSerialPorts] = useState<SerialPortDescriptor[]>([]);
  const [selectedPort, setSelectedPort] = useState(savedPort);
  const [hardwareProfile, setHardwareProfile] = useState<SerialHardwareProfile>(savedProfile);
  const [baudRate, setBaudRate] = useState<number | "auto">(savedBaudRate);
  const [probingRate, setProbingRate] = useState<number | null>(null);
  const ready = snapshot?.phase === "ready";

  useEffect(() => {
    if (open) setSection(initialSection);
  }, [initialSection, open]);

  async function selectMode(next: ConnectionMode) {
    setMode(next);
    setError(null);
    if (next !== "serial") return;
    try {
      const ports = await bridge.listSerialPorts();
      setSerialPorts(ports);
      setSelectedPort((current) => current && ports.some(({ portName }) => portName === current)
        ? current
        : ports[0]?.portName || "");
      if (ports.length === 0) {
        setError("未发现可用串口，请检查无线模块或 Windows 蓝牙配对");
      }
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
      if (ready) {
        const result = await bridge.disconnect();
        if (result.status === "failed") setError(result.message);
      } else if (mode === "simulator") {
        const result = await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
        if (result.status === "failed") setError(result.message);
      } else {
        const result = await connectSerialWithProbe(
          bridge,
          { hardwareProfile, portName: selectedPort, baudRate },
          setProbingRate,
        );
        if (result.operation.status === "failed") {
          setError(`${result.operation.message}（已尝试 ${result.attemptedBaudRates.join("、")} baud）`);
        } else if (result.baudRate !== null) {
          setBaudRate(result.baudRate);
          saveSerialConnection(hardwareProfile, selectedPort, result.baudRate);
        }
      }
    } catch (reason) {
      setError(errorMessage(reason));
    } finally {
      setBusy(false);
      setProbingRate(null);
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

  function changeHardwareProfile(next: SerialHardwareProfile) {
    setHardwareProfile(next);
    setBaudRate("auto");
    setError(null);
  }

  const pendingEndpoint = mode === "simulator"
    ? "内置模拟器 · 等待连接"
    : selectedPort
      ? `${selectedPort} @ ${baudRate === "auto" ? "自动探测" : baudRate}`
      : "等待选择 COM";
  const statusLabel = hydrated ? connectionLabel(snapshot) : "载入状态";
  const endpoint = snapshot?.transportIdentity
    ? endpointLabel(snapshot.transportIdentity.endpoint)
    : pendingEndpoint;

  return (
    <Drawer
      description="连接设置、硬件安全说明与当前设备信息"
      onOpenChange={onOpenChange}
      open={open}
      title="设备连接"
    >
      <div className="app-surface flex items-center gap-3 p-3">
        <span className={ready ? "text-(--success)" : "text-(--warning)"}>
          {ready
            ? <PlugsConnected aria-hidden="true" size={20} weight="fill" />
            : <LinkBreak aria-hidden="true" size={20} />}
        </span>
        <div className="min-w-0 flex-1">
          <output aria-live="polite" className="block text-sm font-semibold">{statusLabel}</output>
          <span className="data-value block truncate text-[11px] text-(--text-muted)">{endpoint}</span>
        </div>
        <Badge>{ready ? "设备在线" : "本地演示权限"}</Badge>
      </div>

      <nav aria-label="设备连接分区" className="mt-4 grid grid-cols-3 gap-2">
        <SectionButton active={section === "connection"} onClick={() => setSection("connection")}>连接</SectionButton>
        <SectionButton active={section === "guide"} onClick={() => setSection("guide")}>硬件指南</SectionButton>
        <SectionButton active={section === "preferences"} onClick={() => setSection("preferences")}>偏好</SectionButton>
      </nav>

      <div className="mt-4">
        {section === "connection" && (
          <section aria-label="连接设置" className="space-y-3">
            <ConnectionField label="连接方式">
              <Select
                aria-label="连接方式"
                disabled={ready || busy}
                onChange={(event) => void selectMode(event.currentTarget.value as ConnectionMode)}
                value={mode}
              >
                <option value="simulator">模拟器体验</option>
                <option value="serial">真实串口</option>
              </Select>
            </ConnectionField>
            {mode === "serial" && (
              <>
                <ConnectionField label="硬件类型">
                  <Select
                    aria-label="硬件类型"
                    disabled={ready || busy}
                    onChange={(event) => changeHardwareProfile(event.currentTarget.value as SerialHardwareProfile)}
                    value={hardwareProfile}
                  >
                    {(Object.entries(HARDWARE_PROFILES) as Array<
                      [SerialHardwareProfile, (typeof HARDWARE_PROFILES)[SerialHardwareProfile]]
                    >).map(([value, profile]) => (
                      <option key={value} value={value}>{profile.label}</option>
                    ))}
                  </Select>
                </ConnectionField>
                <ConnectionField label="串口">
                  <Select
                    aria-label="选择串口"
                    className="data-value"
                    disabled={ready || busy || serialPorts.length === 0}
                    onChange={(event) => setSelectedPort(event.currentTarget.value)}
                    value={selectedPort}
                  >
                    <option value="">选择 COM</option>
                    {serialPorts.map((port) => (
                      <option key={port.portName} value={port.portName}>
                        {port.portName} · {port.portKind === "bluetooth" ? "蓝牙 · " : ""}{port.displayName}
                      </option>
                    ))}
                  </Select>
                </ConnectionField>
                <ConnectionField label="波特率">
                  <Select
                    aria-label="串口波特率"
                    className="data-value"
                    disabled={ready || busy}
                    onChange={(event) => setBaudRate(event.currentTarget.value === "auto"
                      ? "auto"
                      : Number(event.currentTarget.value))}
                    value={baudRate}
                  >
                    <option value="auto">自动探测</option>
                    {SUPPORTED_SERIAL_BAUD_RATES.map((rate) => (
                      <option key={rate} value={rate}>{rate}</option>
                    ))}
                  </Select>
                </ConnectionField>
                {bridge.serialAccessMode === "browser" && (
                  <Button
                    className="w-full"
                    disabled={ready || busy}
                    onClick={() => void authorizeBrowserPort()}
                    variant="secondary"
                  >
                    授权浏览器串口
                  </Button>
                )}
              </>
            )}
            <Button
              className="w-full"
              disabled={busy || (!ready && mode === "serial" && selectedPort === "")}
              onClick={() => void toggleConnection()}
              variant={ready ? "secondary" : "primary"}
            >
              {ready
                ? "断开设备"
                : probingRate !== null
                  ? `正在探测 ${probingRate}`
                  : mode === "serial" ? "连接真实设备" : "连接模拟器"}
            </Button>
            {(error ?? eventError) && <Alert>{error ?? eventError}</Alert>}
          </section>
        )}
        {section === "guide" && <HardwareConnectionGuide profile={hardwareProfile} />}
        {section === "preferences" && <VehicleSwitcher />}
      </div>

      <FirmwareFlashEntry
        firmwareVersion={snapshot?.firmwareVersion ?? null}
        state={{ kind: "unavailable" }}
      />
    </Drawer>
  );
}

function SectionButton({
  active,
  children,
  onClick,
}: {
  active: boolean;
  children: string;
  onClick: () => void;
}) {
  return (
    <Button
      aria-current={active ? "page" : undefined}
      className={active ? "border-(--interactive) text-(--interactive)" : undefined}
      onClick={onClick}
      size="sm"
      variant="secondary"
    >
      {children}
    </Button>
  );
}

function ConnectionField({ children, label }: { children: ReactNode; label: string }) {
  return (
    <label className="block text-xs font-medium text-(--text-muted)">
      <span className="mb-1.5 block">{label}</span>
      {children}
    </label>
  );
}

function errorMessage(reason: unknown): string {
  if (reason instanceof Error) return reason.message;
  if (typeof reason === "object" && reason !== null && "message" in reason && typeof reason.message === "string") {
    return reason.message;
  }
  return "连接操作失败";
}
