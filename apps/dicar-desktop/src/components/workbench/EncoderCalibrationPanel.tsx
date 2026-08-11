import { Gauge } from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import type { ParameterSnapshot } from "../../domain/types";
import { useCollaborationStore } from "../../stores/collaborationStore";
import { Alert } from "../ui/alert";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { Select } from "../ui/select";
import { TypedParameterControl, writeDenial } from "./TypedParameterControl";

const required = ["encoder.left.ppr", "encoder.right.ppr", "encoder.quadrature_multiplier"] as const;

export function EncoderCalibrationPanel({ records }: { records: ParameterSnapshot[] }) {
  const bridge = useDesktopBridge();
  const profile = useCollaborationStore((state) => state.profile);
  const byName = useMemo(() => new Map(records.map((record) => [record.machineName, record])), [records]);
  const left = byName.get("encoder.left.ppr");
  const right = byName.get("encoder.right.ppr");
  const multiplier = byName.get("encoder.quadrature_multiplier");
  const [leftDraft, setLeftDraft] = useState(numberValue(left));
  const [rightDraft, setRightDraft] = useState(numberValue(right));
  const [multiplierDraft, setMultiplierDraft] = useState(numberValue(multiplier) || 1);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const missing = required.filter((name) => !byName.has(name));
  const remaining = records.filter((record) => !required.includes(record.machineName as typeof required[number]));
  const denial = left ? writeDenial(left, profile.role, profile.leaseActive) : "设备清单缺少左编码器 PPR";

  async function applyBaseline() {
    if (!left || !right || !multiplier || denial !== null) return;
    setBusy(true);
    for (const [record, value] of [[left, { kind: "u32", value: leftDraft }], [right, { kind: "u32", value: rightDraft }], [multiplier, { kind: "enum", value: multiplierDraft }]] as const) {
      const result = await bridge.writeParameter(record.paramId, value);
      if (result.status !== "succeeded") { setMessage(result.message); setBusy(false); return; }
    }
    setMessage("编码器 PPR 与倍频已由设备确认");
    setBusy(false);
  }

  return <div className="space-y-3">
    <section className="rounded-[var(--radius)] border border-(--interactive) bg-[color-mix(in_srgb,var(--interactive)_6%,var(--surface-raised))] p-4">
      <div className="mb-4 flex items-center gap-2"><Gauge className="text-(--interactive)" size={20} /><div><h3 className="m-0 text-sm">编码器标定基准</h3><p className="m-0 mt-0.5 text-[11px] text-(--text-muted)">PPR 是每转脉冲数；有效 CPR = PPR × 正交倍频</p></div></div>
      {missing.map((name) => <div className="mb-3" key={name}><Alert>兼容性警告：设备清单缺少 {name}</Alert></div>)}
      <div className="grid gap-3 sm:grid-cols-3">
        {left && <Field label="左编码器 PPR"><Input aria-label="左编码器 PPR" min={1} onChange={(event) => setLeftDraft(Number(event.currentTarget.value))} type="number" value={leftDraft} /></Field>}
        {right && <Field label="右编码器 PPR"><Input aria-label="右编码器 PPR" min={1} onChange={(event) => setRightDraft(Number(event.currentTarget.value))} type="number" value={rightDraft} /></Field>}
        {multiplier && <Field label="正交倍频"><Select aria-label="正交倍频" onChange={(event) => setMultiplierDraft(Number(event.currentTarget.value))} value={multiplierDraft}>{[1, 2, 4].map((value) => <option key={value} value={value}>×{value}</option>)}</Select></Field>}
        {left && multiplier && <Field label="左有效 CPR"><Input aria-label="左有效 CPR" aria-readonly="true" readOnly value={String(leftDraft * multiplierDraft)} /></Field>}
        {right && multiplier && <Field label="右有效 CPR"><Input aria-label="右有效 CPR" aria-readonly="true" readOnly value={String(rightDraft * multiplierDraft)} /></Field>}
      </div>
      <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-(--border) pt-3"><p aria-live="polite" className="m-0 text-[10px] text-(--text-muted)">{message ?? denial ?? "三个字段将逐项写入 RAM；CPR 仅在本地计算，不单独下发。"}</p><Button disabled={busy || missing.length > 0 || denial !== null} onClick={() => void applyBaseline()} size="sm">应用编码器基准到 RAM</Button></div>
    </section>
    {remaining.map((record) => <TypedParameterControl key={record.paramId} record={record} />)}
  </div>;
}

function numberValue(record: ParameterSnapshot | undefined): number {
  return record && record.ramValue.kind !== "bool" ? record.ramValue.value : 0;
}

function Field({ children, label }: { children: React.ReactNode; label: string }) {
  return <div><Label>{label}</Label>{children}</div>;
}
