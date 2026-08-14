import { useEffect } from "react";
import type { DesktopBridge } from "../bridge/desktopBridge";
import { useCollaborationStore } from "../stores/collaborationStore";
import { useConnectionStore } from "../stores/connectionStore";
import type { RecordingController } from "../stores/recordingStore";
import { useWorkspaceStore } from "../stores/workspaceStore";

export function useBridgeSubscription(bridge: DesktopBridge, recordingController: RecordingController): void {
  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    useConnectionStore.getState().reset();
    useWorkspaceStore.getState().reset();
    useCollaborationStore.getState().reset();
    void recordingController.initialize().catch(() => undefined);

    void bridge.getSnapshot().then((snapshot) => {
      if (disposed) return;
      recordingController.setSnapshot(snapshot);
      useConnectionStore.getState().setInitialSnapshot(snapshot);
      useCollaborationStore.getState().setProfile(snapshot.accessProfile);
    });
    void bridge.subscribe((event) => {
      if (disposed) return;
      recordingController.acceptEvent(event);
      useConnectionStore.getState().acceptEvent(event);
      useWorkspaceStore.getState().acceptEvent(event);
      if (event.event === "snapshotChanged") {
        useCollaborationStore.getState().setProfile(event.data.accessProfile);
      }
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unsubscribe = cleanup;
    });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [bridge, recordingController]);
}
