import { useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import type { ParameterSnapshot } from "../../domain/types";
import { useTuningSnapshotStore } from "../../stores/tuningSnapshotStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import { captureTuningSnapshot } from "../../tuning/snapshots";
import { Button } from "../ui/button";
import { formatValue } from "./TypedParameterControl";

export function CommitReviewDialog({ open, records, onClose }: { open: boolean; records: ParameterSnapshot[]; onClose: () => void }) {
  const bridge = useDesktopBridge();
  const profileId = useVehicleProfileStore((state) => state.selectedProfileId);
  const saveSnapshot = useTuningSnapshotStore((state) => state.saveSnapshot);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!open) return null;

  async function commit() {
    setBusy(true);
    setError(null);
    const result = await bridge.commitParameters();
    if (result.status === "succeeded") {
      // 规格 §14：设备确认写入 Flash 后自动生成固化记录，关联 Generation。
      const committed = await bridge.getSnapshot();
      const record = captureTuningSnapshot(committed, {
        name: `固化记录 ${new Date().toLocaleString("zh-CN")}`,
        note: `固化 ${records.length} 项参数`,
        origin: "commit",
        profileId,
        nowMs: Date.now(),
        id: crypto.randomUUID(),
      });
      if (record !== null) saveSnapshot(record);
    }
    setBusy(false);
    if (result.status === "succeeded") onClose();
    else setError(result.message);
  }

  return <div className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <section aria-labelledby="commit-title" aria-modal="true" className="max-h-[85vh] w-full max-w-3xl overflow-auto rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) shadow-2xl" role="dialog">
      <header className="border-b border-(--border) p-4"><h2 className="m-0 text-base" id="commit-title">固化参数修改</h2><p className="m-0 mt-1 text-xs text-(--text-muted)">确认后由设备校验 revision 与 CRC，再一次性写入 Flash。</p></header>
      <div className="overflow-x-auto p-4"><table className="w-full border-collapse text-left text-xs"><thead><tr className="border-b border-(--border) text-(--text-muted)"><th className="p-2">参数</th><th className="p-2">Flash</th><th className="p-2">RAM</th><th className="p-2">Revision</th></tr></thead><tbody>{records.map((record) => <tr className="border-b border-(--border)" key={record.paramId}><td className="p-2 font-medium">{record.displayName}</td><td className="p-2 font-mono">{record.persistedValue ? formatValue(record.persistedValue) : "—"}</td><td className="p-2 font-mono text-(--interactive)">{formatValue(record.ramValue)} {record.unit}</td><td className="p-2 font-mono">{record.revision}</td></tr>)}</tbody></table></div>
      {error && <p aria-live="assertive" className="mx-4 text-xs text-(--danger)">{error}</p>}
      <footer className="flex justify-end gap-2 border-t border-(--border) p-4"><Button onClick={onClose} variant="secondary">取消</Button><Button disabled={busy || records.length === 0} onClick={() => void commit()}>固化到 Flash</Button></footer>
    </section>
  </div>;
}
