import { useEffect } from "react";
import type { DesktopBridge } from "../bridge/desktopBridge";
import { useCollaborationStore } from "../stores/collaborationStore";
import { useConnectionStore } from "../stores/connectionStore";
import { useWorkspaceStore } from "../stores/workspaceStore";

export function useBridgeSubscription(bridge: DesktopBridge): void {
  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    useConnectionStore.getState().reset();
    useWorkspaceStore.getState().reset();
    useCollaborationStore.getState().reset();

    void bridge.getSnapshot().then((snapshot) => {
      if (disposed) return;
      useConnectionStore.getState().setInitialSnapshot(snapshot);
      useCollaborationStore.getState().setProfile(snapshot.accessProfile);
    });
    void bridge.subscribe((event) => {
      if (disposed) return;
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
  }, [bridge]);
}
