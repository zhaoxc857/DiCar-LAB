import { createContext, useContext, useState, type PropsWithChildren } from "react";
import type { AiPlatform } from "../ai/aiPlatform";
import { TauriAiPlatform, UnavailableAiPlatform } from "../ai/aiPlatform";
import type { DesktopBridge } from "../bridge/desktopBridge";
import { MockBridge } from "../bridge/mockBridge";
import { TauriBridge } from "../bridge/tauriBridge";
import { WebSerialBridge, type BrowserSerial } from "../bridge/webSerialBridge";
import { useBridgeSubscription } from "../hooks/useBridgeSubscription";

const DesktopBridgeContext = createContext<DesktopBridge | null>(null);
const AiPlatformContext = createContext<AiPlatform | null>(null);

type AppProvidersProps = PropsWithChildren<{
  bridge?: DesktopBridge;
  aiPlatform?: AiPlatform;
}>;

export function AppProviders({ bridge, aiPlatform, children }: AppProvidersProps) {
  const [resolvedBridge] = useState<DesktopBridge>(() => bridge ?? createDefaultBridge());
  const [resolvedAiPlatform] = useState<AiPlatform>(() => aiPlatform ?? createDefaultAiPlatform());
  return (
    <AiPlatformContext.Provider value={resolvedAiPlatform}>
      <DesktopBridgeContext.Provider value={resolvedBridge}>
        <BridgeSubscription bridge={resolvedBridge} />
        {children}
      </DesktopBridgeContext.Provider>
    </AiPlatformContext.Provider>
  );
}

export function useAiPlatform(): AiPlatform {
  const platform = useContext(AiPlatformContext);
  if (platform === null) {
    throw new Error("useAiPlatform 必须在 AppProviders 内使用");
  }
  return platform;
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
  if (isTauriShell()) {
    return new TauriBridge();
  }
  if (typeof navigator !== "undefined" && "serial" in navigator) {
    return new WebSerialBridge((navigator as Navigator & { serial: BrowserSerial }).serial);
  }
  return new MockBridge();
}

function createDefaultAiPlatform(): AiPlatform {
  return isTauriShell() ? new TauriAiPlatform() : new UnavailableAiPlatform();
}

function isTauriShell(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
