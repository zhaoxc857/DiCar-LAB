import { createContext, useContext, useState, type PropsWithChildren } from "react";
import type { DesktopBridge } from "../bridge/desktopBridge";
import { MockBridge } from "../bridge/mockBridge";
import { TauriBridge } from "../bridge/tauriBridge";
import { WebSerialBridge, type BrowserSerial } from "../bridge/webSerialBridge";
import { useBridgeSubscription } from "../hooks/useBridgeSubscription";

const DesktopBridgeContext = createContext<DesktopBridge | null>(null);

type AppProvidersProps = PropsWithChildren<{
  bridge?: DesktopBridge;
}>;

export function AppProviders({ bridge, children }: AppProvidersProps) {
  const [resolvedBridge] = useState<DesktopBridge>(() => bridge ?? createDefaultBridge());
  return (
    <DesktopBridgeContext.Provider value={resolvedBridge}>
      <BridgeSubscription bridge={resolvedBridge} />
      {children}
    </DesktopBridgeContext.Provider>
  );
}

function BridgeSubscription({ bridge }: { bridge: DesktopBridge }) {
  useBridgeSubscription(bridge);
  return null;
}

export function useDesktopBridge(): DesktopBridge {
  const bridge = useContext(DesktopBridgeContext);
  if (bridge === null) {
    throw new Error("useDesktopBridge 必须在 AppProviders 内使用");
  }
  return bridge;
}

function createDefaultBridge(): DesktopBridge {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return new TauriBridge();
  }
  if (typeof navigator !== "undefined" && "serial" in navigator) {
    return new WebSerialBridge((navigator as Navigator & { serial: BrowserSerial }).serial);
  }
  return new MockBridge();
}
