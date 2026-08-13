import { ArrowCounterClockwise, DownloadSimple, FloppyDisk, Trash } from "@phosphor-icons/react";
import { useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import { useCollaborationStore } from "../../stores/collaborationStore";
import { useConnectionStore } from "../../stores/connectionStore";
import { useTuningSnapshotStore } from "../../stores/tuningSnapshotStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import {
  captureTuningSnapshot,
  diffTuningSnapshot,
  snapshotExportJson,
  DISPOSITION_LABELS,
  type SnapshotDiff,
  type TuningSnapshot,
} from "../../tuning/snapshots";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { formatValue } from "./TypedParameterControl";

type ApplyOutcome = { applied: number; failed: string[] } | null;

export function SnapshotManagerDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const bridge = useDesktopBridge();
  const snapshot = useConnectionStore((state) => state.snapshot);
  const profile = useCollaborationStore((state) => state.profile);
  const profileId = useVehicleProfileStore((state) => state.selectedProfileId);
  const snapshots = useTuningSnapshotStore((state) => state.snapshots);
  const issues = useTuningSnapshotStore((state) => state.issues);
  const saveSnapshot = useTuningSnapshotStore((state) => state.saveSnapshot);
  const removeSnapshot = useTuningSnapshotStore((state) => state.removeSnapshot);
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [reviewing, setReviewing] = useState<{ snapshot: TuningSnapshot; diff: SnapshotDiff } | null>(null);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<ApplyOutcome>(null);
  if (!open) return null;

  const parameters = snapshot?.parameters ?? [];
  const saveDenial =
    profile.role === "observer"
      ? "仅观察者不能创建参数方案"
      : parameters.length === 0
        ? "连接设备并同步参数后才能保存方案"
        : null;
  const applyDenial =
    profile.role === "observer"
      ? "仅观察者不能应用参数方案"
      : !profile.leaseActive
        ? "当前车辆控制权未激活"
        : parameters.length === 0
          ? "连接设备并同步参数后才能应用方案"
          : null;

  function save() {
    if (saveDenial !== null || snapshot === null) return;
    const trimmed = name.trim();
    if (trimmed.length === 0) {
      setMessage("请先给方案起一个名称");
      return;
    }
    const captured = captureTuningSnapshot(snapshot, {
      name: trimmed,
      note: note.trim(),
      origin: "manual",
      profileId,
      nowMs: Date.now(),
      id: crypto.randomUUID(),
    });
    if (captured === null) {
      setMessage("当前没有可保存的参数");
      return;
    }
    const result = saveSnapshot(captured);
    setMessage(result.status === "saved" ? `已保存「${trimmed}」（${captured.entries.length} 项参数）` : result.message);
    if (result.status === "saved") {
      setName("");
      setNote("");
    }
  }

  function beginReview(target: TuningSnapshot) {
    setOutcome(null);
    setReviewing({ snapshot: target, diff: diffTuningSnapshot(target, parameters) });
  }

  async function applyReviewed() {
    if (reviewing === null || applyDenial !== null) return;
    setBusy(true);
    const failed: string[] = [];
    let applied = 0;
    for (const { entry } of reviewing.diff.applicable) {
      const result = await bridge.writeParameter(entry.paramId, entry.value);
      if (result.status === "succeeded") applied += 1;
      else failed.push(`${entry.displayName}：${result.message}`);
    }
    setBusy(false);
    setOutcome({ applied, failed });
  }

  function exportSnapshot(target: TuningSnapshot) {
    const blob = new Blob([snapshotExportJson(target)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `dicar-snapshot-${target.name.replace(/[\\/:*?"<>|\s]+/g, "-")}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        aria-labelledby="snapshot-manager-title"
        aria-modal="true"
        className="max-h-[85vh] w-full max-w-3xl overflow-auto rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) shadow-2xl"
        role="dialog"
      >
        <header className="border-b border-(--border) p-4">
          <h2 className="m-0 text-base" id="snapshot-manager-title">
            参数方案
          </h2>
          <p className="m-0 mt-1 text-xs text-(--text-muted)">
            保存当前 RAM 参数为可恢复方案；应用时按稳定 ID 匹配，缺失、类型变化和越界项只列出、不自动写入。
          </p>
        </header>

        {reviewing === null ? (
          <div className="space-y-4 p-4">
            <section className="rounded-[var(--radius)] border border-(--border) p-3">
              <h3 className="m-0 text-sm">保存当前方案</h3>
              <div className="mt-2 grid gap-2 sm:grid-cols-[1fr_1fr_auto] sm:items-end">
                <div>
                  <Label htmlFor="snapshot-name">方案名称</Label>
                  <Input
                    disabled={saveDenial !== null}
                    id="snapshot-name"
                    onChange={(event) => setName(event.currentTarget.value)}
                    placeholder="例如：周三直道最优"
                    value={name}
                  />
                </div>
                <div>
                  <Label htmlFor="snapshot-note">说明（可选）</Label>
                  <Input
                    disabled={saveDenial !== null}
                    id="snapshot-note"
                    onChange={(event) => setNote(event.currentTarget.value)}
                    placeholder="记录赛道、电压等背景"
                    value={note}
                  />
                </div>
                <Button disabled={saveDenial !== null} onClick={save} size="sm">
                  <FloppyDisk size={15} />
                  保存方案
                </Button>
              </div>
              {saveDenial !== null && <p className="m-0 mt-2 text-xs text-(--warning)">{saveDenial}</p>}
              {message !== null && (
                <p aria-live="polite" className="m-0 mt-2 text-xs text-(--text-muted)">
                  {message}
                </p>
              )}
            </section>

            {issues.length > 0 && <p className="m-0 text-xs text-(--warning)">{issues.join("；")}</p>}

            <section>
              <h3 className="m-0 mb-2 text-sm">已保存方案（{snapshots.length}）</h3>
              {snapshots.length === 0 ? (
                <p className="m-0 text-xs text-(--text-muted)">还没有保存的方案。固化成功时也会自动生成固化记录。</p>
              ) : (
                <ul className="m-0 list-none space-y-2 p-0">
                  {snapshots.map((entry) => (
                    <li
                      className="flex flex-wrap items-center justify-between gap-2 rounded-[var(--radius)] border border-(--border) p-3"
                      key={entry.id}
                    >
                      <div className="min-w-0">
                        <p className="m-0 truncate text-sm font-medium">
                          {entry.name}
                          <span
                            className={`ml-2 rounded border px-1.5 py-0.5 text-[10px] ${entry.origin === "commit" ? "border-(--interactive) text-(--interactive)" : "border-(--border) text-(--text-muted)"}`}
                          >
                            {entry.origin === "commit" ? `固化记录 · Gen ${entry.storageGeneration ?? "?"}` : "手动保存"}
                          </span>
                        </p>
                        <p className="m-0 mt-1 text-[11px] text-(--text-muted)">
                          {new Date(entry.createdAtMs).toLocaleString("zh-CN")} · {entry.entries.length} 项参数
                          {entry.note.length > 0 ? ` · ${entry.note}` : ""}
                        </p>
                      </div>
                      <div className="flex gap-1.5">
                        <Button disabled={applyDenial !== null} onClick={() => beginReview(entry)} size="sm" variant="secondary">
                          <ArrowCounterClockwise size={14} />
                          应用
                        </Button>
                        <Button onClick={() => exportSnapshot(entry)} size="sm" variant="secondary">
                          <DownloadSimple size={14} />
                          导出
                        </Button>
                        <Button
                          aria-label={`删除方案 ${entry.name}`}
                          onClick={() => removeSnapshot(entry.id)}
                          size="sm"
                          variant="secondary"
                        >
                          <Trash size={14} />
                        </Button>
                      </div>
                    </li>
                  ))}
                </ul>
              )}
              {applyDenial !== null && snapshots.length > 0 && (
                <p className="m-0 mt-2 text-xs text-(--warning)">{applyDenial}</p>
              )}
            </section>
          </div>
        ) : (
          <div className="p-4">
            <h3 className="m-0 text-sm">应用「{reviewing.snapshot.name}」</h3>
            <p className="m-0 mt-1 text-xs text-(--text-muted)">
              {reviewing.diff.applicable.length} 项将写入 RAM，{reviewing.diff.blocked.length} 项被跳过。写入不会自动固化到
              Flash。
            </p>
            <div className="mt-3 overflow-x-auto">
              <table className="w-full border-collapse text-left text-xs">
                <thead>
                  <tr className="border-b border-(--border) text-(--text-muted)">
                    <th className="p-2">参数</th>
                    <th className="p-2">当前 RAM</th>
                    <th className="p-2">方案值</th>
                    <th className="p-2">处理</th>
                  </tr>
                </thead>
                <tbody>
                  {reviewing.diff.entries.map(({ entry, disposition, currentValue }) => (
                    <tr className="border-b border-(--border)" key={entry.paramId}>
                      <td className="p-2 font-medium">{entry.displayName}</td>
                      <td className="p-2 font-mono">{currentValue === null ? "—" : formatValue(currentValue)}</td>
                      <td className="p-2 font-mono text-(--interactive)">
                        {formatValue(entry.value)} {entry.unit}
                      </td>
                      <td
                        className={`p-2 ${disposition === "apply" ? "text-(--interactive)" : disposition === "match" ? "text-(--text-muted)" : "text-(--warning)"}`}
                      >
                        {DISPOSITION_LABELS[disposition]}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {outcome !== null && (
              <p aria-live="polite" className={`m-0 mt-3 text-xs ${outcome.failed.length > 0 ? "text-(--warning)" : "text-(--text-muted)"}`}>
                已写入 {outcome.applied} 项
                {outcome.failed.length > 0 ? `；失败 ${outcome.failed.length} 项：${outcome.failed.join("；")}` : "，设备已确认"}
              </p>
            )}
            <footer className="mt-4 flex justify-end gap-2">
              <Button onClick={() => setReviewing(null)} variant="secondary">
                返回列表
              </Button>
              <Button
                disabled={busy || applyDenial !== null || reviewing.diff.applicable.length === 0 || outcome !== null}
                onClick={() => void applyReviewed()}
              >
                写入 {reviewing.diff.applicable.length} 项到 RAM
              </Button>
            </footer>
          </div>
        )}

        <footer className="flex justify-end border-t border-(--border) p-4">
          <Button onClick={onClose} variant="secondary">
            关闭
          </Button>
        </footer>
      </section>
    </div>
  );
}
