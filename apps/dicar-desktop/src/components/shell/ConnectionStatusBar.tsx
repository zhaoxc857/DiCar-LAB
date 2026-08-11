import { CircleNotch, LinkBreak, PlugsConnected } from "@phosphor-icons/react";
import { useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import { connectionLabel, useConnectionStore } from "../../stores/connectionStore";
import { Alert } from "../ui/alert";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";

export function ConnectionStatusBar() {
  const bridge = useDesktopBridge();
  const snapshot = useConnectionStore((state) => state.snapshot);
  const hydrated = useConnectionStore((state) => state.hydrated);
  const eventError = useConnectionStore((state) => state.eventError);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ready = snapshot?.phase === "ready";

  async function toggleConnection() {
    setBusy(true);
    setError(null);
    try {
      const result = ready
        ? await bridge.disconnect()
        : await bridge.connect({ kind: "simulator", address: "127.0.0.1:7100" });
      if (result.status === "failed") setError(result.message);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "连接操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <section aria-label="连接状态" className="flex flex-wrap items-center justify-between gap-3 border-b border-(--border) bg-(--surface) px-4 py-2.5 lg:px-6">
        <div className="flex min-w-0 items-center gap-3">
          <span className={ready ? "text-(--success)" : "text-(--warning)"}>
            {busy ? <CircleNotch className="animate-spin" aria-hidden="true" size={20} /> : ready ? <PlugsConnected aria-hidden="true" size={20} /> : <LinkBreak aria-hidden="true" size={20} />}
          </span>
          <div>
            <output aria-live="polite" className="block text-sm font-semibold">{hydrated ? connectionLabel(snapshot) : "载入状态"}</output>
            <p className="m-0 mt-0.5 font-mono text-[11px] text-(--text-muted)">{snapshot?.transportIdentity?.endpoint.address ?? "TCP 127.0.0.1:7100 · 等待连接"}</p>
          </div>
          <Badge className="hidden sm:inline-flex">本地演示权限</Badge>
          <Badge className="hidden sm:inline-flex">单一活动主机</Badge>
        </div>
        <Button disabled={busy} onClick={() => void toggleConnection()} size="sm" variant={ready ? "secondary" : "primary"}>
          {ready ? "断开设备" : "连接模拟器"}
        </Button>
      </section>
      {(error ?? eventError) && <div className="px-4 pt-3 lg:px-6"><Alert>{error ?? eventError}</Alert></div>}
    </>
  );
}
