import { createContext, useContext, useState, useSyncExternalStore, type PropsWithChildren } from "react";
import type { AiPlatform } from "../ai/aiPlatform";
import { TauriAiPlatform, UnavailableAiPlatform } from "../ai/aiPlatform";
import type { DesktopBridge } from "../bridge/desktopBridge";
import { MockBridge } from "../bridge/mockBridge";
import { TauriBridge } from "../bridge/tauriBridge";
import { WebSerialBridge, type BrowserSerial } from "../bridge/webSerialBridge";
import type { FirmwareFlashPlatform } from "../firmware/firmwarePlatform";
import {
  TauriFirmwareFlashPlatform,
  UnavailableFirmwareFlashPlatform,
} from "../firmware/firmwarePlatform";
import { useBridgeSubscription } from "../hooks/useBridgeSubscription";
import { getDefaultRecordingController, type RecordingController } from "../stores/recordingStore";

const DesktopBridgeContext = createContext<DesktopBridge | null>(null);
const AiPlatformContext = createContext<AiPlatform | null>(null);
const FirmwareFlashPlatformContext = createContext<FirmwareFlashPlatform | null>(null);
const RecordingControllerContext = createContext<RecordingController | null>(null);

type AppProvidersProps = PropsWithChildren<{
  bridge?: DesktopBridge;
  aiPlatform?: AiPlatform;
  firmwarePlatform?: FirmwareFlashPlatform;
  recordingController?: RecordingController;
}>;

export function AppProviders({
  bridge,
  aiPlatform,
  firmwarePlatform,
  recordingController,
  children,
}: AppProvidersProps) {
  const [resolvedBridge] = useState<DesktopBridge>(() => bridge ?? createDefaultBridge());
  const [resolvedAiPlatform] = useState<AiPlatform>(() => aiPlatform ?? createDefaultAiPlatform());
  const [resolvedFirmwarePlatform] = useState<FirmwareFlashPlatform>(
    () => firmwarePlatform ?? createDefaultFirmwarePlatform(),
  );
  const [resolvedRecordingController] = useState<RecordingController>(
    () => recordingController ?? getDefaultRecordingController(),
  );
  return (
    <FirmwareFlashPlatformContext.Provider value={resolvedFirmwarePlatform}>
      <AiPlatformContext.Provider value={resolvedAiPlatform}>
        <RecordingControllerContext.Provider value={resolvedRecordingController}>
          <DesktopBridgeContext.Provider value={resolvedBridge}>
            <BridgeSubscription bridge={resolvedBridge} recordingController={resolvedRecordingController} />
            {children}
          </DesktopBridgeContext.Provider>
        </RecordingControllerContext.Provider>
      </AiPlatformContext.Provider>
    </FirmwareFlashPlatformContext.Provider>
  );
}

export function useFirmwareFlashPlatform(): FirmwareFlashPlatform {
  const platform = useContext(FirmwareFlashPlatformContext);
  if (platform === null) {
    throw new Error("useFirmwareFlashPlatform 必须在 AppProviders 内使用");
  }
  return platform;
}

export function useAiPlatform(): AiPlatform {
  const platform = useContext(AiPlatformContext);
  if (platform === null) {
    throw new Error("useAiPlatform 必须在 AppProviders 内使用");
  }
  return platform;
}

function BridgeSubscription({ bridge, recordingController }: { bridge: DesktopBridge; recordingController: RecordingController }) {
  useBridgeSubscription(bridge, recordingController);
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

export function useRecordingController(): RecordingController {
  const controller = useContext(RecordingControllerContext);
  if (controller === null) {
    throw new Error("useRecordingController 必须在 AppProviders 内使用");
  }
  return controller;
}

export function useRecordingControllerState() {
  const controller = useRecordingController();
  return useSyncExternalStore(controller.subscribe, controller.getState, controller.getState);
}

function createDefaultAiPlatform(): AiPlatform {
  return isTauriShell() ? new TauriAiPlatform() : new UnavailableAiPlatform();
}

function createDefaultFirmwarePlatform(): FirmwareFlashPlatform {
  return isTauriShell()
    ? new TauriFirmwareFlashPlatform()
    : new UnavailableFirmwareFlashPlatform();
}

function isTauriShell(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
