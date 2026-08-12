import * as Dialog from "@radix-ui/react-dialog";
import { FileArrowUp, Trash, X } from "@phosphor-icons/react";
import { useState } from "react";
import { useConnectionStore } from "../../stores/connectionStore";
import { useVehicleProfileStore, type ImportProfileResult } from "../../stores/vehicleProfileStore";
import { builtInProfiles } from "../../vehicleProfiles/catalog";
import { resolveVehicleWorkspace } from "../../vehicleProfiles/resolver";
import { Button } from "../ui/button";

export function VehicleProfileManager({ open, onClose }: { open: boolean; onClose: () => void }) {
  const snapshot = useConnectionStore((state) => state.snapshot);
  const userProfiles = useVehicleProfileStore((state) => state.userProfiles);
  const importProfile = useVehicleProfileStore((state) => state.importProfile);
  const removeUserProfile = useVehicleProfileStore((state) => state.removeUserProfile);
  const catalogIssues = useVehicleProfileStore((state) => state.catalogIssues);
  const [result, setResult] = useState<ImportProfileResult | null>(null);
  const [pendingYaml, setPendingYaml] = useState<string | null>(null);
  const [pendingRemoveId, setPendingRemoveId] = useState<string | null>(null);
  const profiles = [...builtInProfiles, ...userProfiles].sort((a, b) => a.profile.vehicle.order - b.profile.vehicle.order || a.profile.vehicle.displayName.localeCompare(b.profile.vehicle.displayName, "zh-CN"));

  async function readFile(file: File | undefined) {
    if (!file) return;
    const yaml = await readFileText(file);
    const next = importProfile(yaml, false);
    setResult(next);
    setPendingYaml(next.status === "needsReplace" ? yaml : null);
  }

  function replace() {
    if (pendingYaml === null) return;
    setResult(importProfile(pendingYaml, true));
    setPendingYaml(null);
  }

  return <Dialog.Root onOpenChange={(next) => { if (!next) onClose(); }} open={open}>
    <Dialog.Portal>
      <Dialog.Overlay className="fixed inset-0 z-40 bg-black/70" />
      <Dialog.Content className="fixed left-1/2 top-1/2 z-50 max-h-[86vh] w-[min(92vw,760px)] -translate-x-1/2 -translate-y-1/2 overflow-auto rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) shadow-2xl">
        <header className="flex items-start justify-between border-b border-(--border) p-4"><div><Dialog.Title className="m-0 text-base">车型配置管理</Dialog.Title><Dialog.Description className="m-0 mt-1 text-xs text-(--text-muted)">导入的 YAML 只组织设备 Manifest 已声明的参数和遥测，不覆盖设备真值。</Dialog.Description></div><Dialog.Close asChild><button aria-label="关闭车型配置管理" className="rounded p-1 text-(--text-muted) hover:text-(--text)" type="button"><X size={18} /></button></Dialog.Close></header>
        <div className="p-4">
          <label className="flex cursor-pointer items-center justify-center gap-2 rounded-[var(--radius)] border border-dashed border-(--interactive) bg-[color-mix(in_srgb,var(--interactive)_6%,transparent)] p-4 text-sm text-(--interactive)"><FileArrowUp size={20} />导入车型 YAML<input accept=".yaml,.yml" aria-label="导入车型 YAML" className="sr-only" onChange={(event) => { void readFile(event.currentTarget.files?.[0]); event.currentTarget.value = ""; }} type="file" /></label>
          {result && <p aria-live="polite" className={`mb-0 mt-3 text-xs ${result.status === "failed" ? "text-(--danger)" : "text-(--success)"}`}>{result.message}</p>}
          {catalogIssues.map((issue) => <p aria-live="polite" className="mb-0 mt-3 text-xs text-(--danger)" key={issue}>{issue}</p>)}
          {result?.status === "needsReplace" && <Button className="mt-3" onClick={replace} size="sm">确认替换 {pendingYaml ? profileName(pendingYaml) : result.profileId}</Button>}
          <div className="mt-4 space-y-2">{profiles.map((entry) => {
            const resolved = snapshot ? resolveVehicleWorkspace(entry.profile, snapshot.parameters, snapshot.telemetryDescriptors) : null;
            const issueCount = resolved?.issues.length ?? 0;
            const confirmingRemove = pendingRemoveId === entry.profile.vehicle.id;
            return <article className="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius)] border border-(--border) bg-(--surface) p-3" key={entry.profile.vehicle.id}><div><div className="flex items-center gap-2"><h3 className="m-0 text-sm">{entry.profile.vehicle.displayName}</h3><span className="rounded border border-(--border) px-1.5 py-0.5 text-[10px] text-(--text-muted)">{entry.source === "builtIn" ? "内置" : "用户"}</span></div><p className="m-0 mt-1 font-mono text-[10px] text-(--text-muted)">{entry.profile.vehicle.id} · {entry.profile.vehicle.type}{snapshot ? ` · ${issueCount} 条兼容性提示` : ""}</p></div>{entry.source === "user" && (confirmingRemove ? <div className="flex gap-2"><Button aria-label={`取消移除 ${entry.profile.vehicle.displayName}`} onClick={() => setPendingRemoveId(null)} size="sm" variant="secondary">取消</Button><Button aria-label={`确认移除 ${entry.profile.vehicle.displayName}`} onClick={() => { removeUserProfile(entry.profile.vehicle.id); setPendingRemoveId(null); }} size="sm" variant="danger"><Trash size={14} />确认移除</Button></div> : <Button aria-label={`移除 ${entry.profile.vehicle.displayName}`} onClick={() => setPendingRemoveId(entry.profile.vehicle.id)} size="sm" variant="danger"><Trash size={14} />移除</Button>)}</article>;
          })}</div>
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>;
}

function profileName(yaml: string): string {
  return yaml.match(/display_name:\s*([^,}\n]+)/)?.[1]?.trim() ?? "车型";
}

function readFileText(file: File): Promise<string> {
  if (typeof file.text === "function") return file.text();
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("无法读取车型配置文件"));
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.readAsText(file);
  });
}
