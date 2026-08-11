import { FloppyDisk, Warning } from "@phosphor-icons/react";
import { useEffect, useId, useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import type { ParameterSnapshot, ParameterValue } from "../../domain/types";
import { useCollaborationStore } from "../../stores/collaborationStore";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { Select } from "../ui/select";
import { Switch } from "../ui/switch";

export function writeDenial(record: ParameterSnapshot, role: string, leaseActive: boolean): string | null {
  if (!record.syncKnown) return "设备状态未知，重新连接并同步后才能修改";
  if (!record.writable) return "该参数由设备声明为只读";
  if (role === "observer") return "仅观察者不能修改参数";
  if (!leaseActive) return "当前车辆控制权未激活";
  return null;
}

export function TypedParameterControl({ record }: { record: ParameterSnapshot }) {
  const bridge = useDesktopBridge();
  const profile = useCollaborationStore((state) => state.profile);
  const inputId = useId();
  const [draft, setDraft] = useState<ParameterValue>(record.ramValue);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const denial = writeDenial(record, profile.role, profile.leaseActive);

  useEffect(() => setDraft(record.ramValue), [record.paramId, record.ramValue]);

  async function submit() {
    if (denial !== null) return;
    setBusy(true);
    const result = await bridge.writeParameter(record.paramId, draft);
    setMessage(result.message);
    setBusy(false);
  }

  return <section className="rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-4">
    <div className="mb-3 flex items-start justify-between gap-3">
      <div><h3 className="m-0 text-sm">{record.displayName}</h3><p className="m-0 mt-1 font-mono text-[11px] text-(--text-muted)">{record.machineName} · ID {record.paramId} · rev {record.revision}</p></div>
      <div className="flex gap-1.5">{record.dirty && <span className="rounded border border-(--warning) px-2 py-0.5 text-[10px] text-(--warning)">RAM 已修改</span>}{record.dangerous && <span className="inline-flex items-center gap-1 rounded border border-(--danger) px-2 py-0.5 text-[10px] text-(--danger)"><Warning size={12} />危险参数</span>}</div>
    </div>
    <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
      <div>
        <Label htmlFor={inputId}>{record.displayName}</Label>
        {renderControl(record, draft, setDraft, inputId, denial !== null)}
        <p className="m-0 mt-1.5 text-[11px] text-(--text-muted)">{record.description ?? valueHint(record)}{record.unit ? ` · 单位 ${record.unit}` : ""}</p>
      </div>
      {denial === null && <Button disabled={busy} onClick={() => void submit()} size="sm"><FloppyDisk size={15} />写入 RAM</Button>}
    </div>
    {denial && <p className="m-0 mt-3 text-xs text-(--warning)">{denial}</p>}
    {(record.lastError ?? message) && <p aria-live="polite" className="m-0 mt-3 text-xs text-(--text-muted)">{record.lastError ?? message}</p>}
    <div className="mt-3 grid grid-cols-2 gap-2 border-t border-(--border) pt-3 text-[11px]"><span className="text-(--text-muted)">RAM <strong className="ml-1 font-mono text-(--text)">{formatValue(record.ramValue)}</strong></span><span className="text-(--text-muted)">Flash <strong className="ml-1 font-mono text-(--text)">{record.persistedValue ? formatValue(record.persistedValue) : "—"}</strong></span></div>
  </section>;
}

function renderControl(record: ParameterSnapshot, draft: ParameterValue, setDraft: (value: ParameterValue) => void, inputId: string, disabled: boolean) {
  if (!record.writable) return <Input aria-label={record.displayName} aria-readonly="true" id={inputId} readOnly value={formatValue(record.ramValue)} />;
  if (draft.kind === "bool") return <div className="flex h-10 items-center gap-3"><Switch aria-label={record.displayName} checked={draft.value} disabled={disabled} id={inputId} onChange={(event) => setDraft({ kind: "bool", value: event.currentTarget.checked })} /><span className="text-sm">{draft.value ? "开启" : "关闭"}</span></div>;
  if (draft.kind === "enum") return <Select aria-label={record.displayName} disabled={disabled} id={inputId} onChange={(event) => setDraft({ kind: "enum", value: Number(event.currentTarget.value) })} value={draft.value}>{(record.enumOptions ?? []).map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</Select>;
  return <Input aria-label={record.displayName} disabled={disabled} id={inputId} max={record.numeric?.max} min={record.numeric?.min} onChange={(event) => setDraft({ kind: draft.kind, value: Number(event.currentTarget.value) } as ParameterValue)} step={record.numeric?.step ?? (draft.kind === "f32" ? "any" : 1)} type="number" value={draft.value} />;
}

function valueHint(record: ParameterSnapshot): string {
  if (record.numeric) return `范围 ${record.numeric.min}–${record.numeric.max}，步进 ${record.numeric.step}`;
  if (record.ramValue.kind === "enum") return "从设备清单声明的枚举项中选择";
  return "修改只在设备 ACK 后成为确认值";
}

export function formatValue(value: ParameterValue): string {
  if (value.kind === "bool") return value.value ? "开启" : "关闭";
  return String(value.value);
}
