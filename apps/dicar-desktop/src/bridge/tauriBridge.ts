import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AccessProfileId,
  AppSnapshot,
  BridgeEvent,
  Endpoint,
  OperationResult,
  ParameterValue,
  TelemetrySubscriptionRequest,
  WindowCloseDecision,
} from "../domain/types";
import type { DesktopBridge } from "./desktopBridge";

export class TauriBridge implements DesktopBridge {
  connect(endpoint: Endpoint): Promise<OperationResult> {
    return invoke("connect", { endpoint });
  }

  disconnect(): Promise<OperationResult> {
    return invoke("disconnect");
  }

  writeParameter(paramId: number, value: ParameterValue): Promise<OperationResult> {
    return invoke("write_parameter", { paramId, value });
  }

  commitParameters(): Promise<OperationResult> {
    return invoke("commit_parameters");
  }

  revertAll(): Promise<OperationResult> {
    return invoke("revert_all");
  }

  undoLast(): Promise<OperationResult> {
    return invoke("undo_last");
  }

  setTelemetrySubscription(request: TelemetrySubscriptionRequest): Promise<OperationResult> {
    return invoke("set_telemetry_subscription", { request });
  }

  setPaused(paused: boolean): Promise<OperationResult> {
    return invoke("set_paused", { paused });
  }

  addMarker(label: string): Promise<OperationResult> {
    return invoke("add_marker", { label });
  }

  resolveWindowClose(
    requestId: number,
    decision: WindowCloseDecision,
  ): Promise<OperationResult> {
    return invoke("resolve_window_close", { requestId, decision });
  }

  selectAccessProfile(profile: AccessProfileId): Promise<OperationResult> {
    return invoke("select_access_profile", { profile });
  }

  getSnapshot(): Promise<AppSnapshot> {
    return invoke("get_snapshot");
  }

  async subscribe(listener: (event: BridgeEvent) => void): Promise<() => void> {
    const onEvent = new Channel<BridgeEvent>();
    onEvent.onmessage = listener;
    await invoke("open_core_channel", { onEvent });
    let closed = false;
    return () => {
      if (closed) return;
      closed = true;
      void invoke("close_core_channel");
    };
  }
}
