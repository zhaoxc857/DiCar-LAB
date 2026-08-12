import type { ParameterSnapshot, TelemetryDescriptor } from "../../domain/types";
import type { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import type { ResolvedControlLoop } from "../../vehicleProfiles/types";
import { Alert } from "../ui/alert";
import { TypedParameterControl } from "./TypedParameterControl";

export function ControlLoopWorkspace({ loop, records, descriptors, buffer }: { loop: ResolvedControlLoop; records: ParameterSnapshot[]; descriptors: TelemetryDescriptor[]; buffer: TelemetryRingBuffer }) {
  const byParamId = new Map(records.map((record) => [record.paramId, record]));
  const roles = [
    { label: "目标", channelId: loop.telemetry.target }, { label: "实际", channelId: loop.telemetry.feedback }, { label: "误差", channelId: loop.telemetry.error },
    ...loop.telemetry.outputs.map((channelId, index) => ({ label: `输出 ${index + 1}`, channelId })),
  ];
  const parameterRecords = [...(loop.targetParamId === null ? [] : [byParamId.get(loop.targetParamId)]), ...loop.gainParamIds.map(({ paramId }) => byParamId.get(paramId))].filter((record): record is ParameterSnapshot => record !== undefined);
  return <section className="min-w-0"><header className="mb-3"><h2 className="m-0 text-sm">{loop.label}</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">{loop.hint ?? "小步调整参数，以设备 ACK 为准，再观察遥测响应。"}</p></header><div className="mb-3 grid gap-2 sm:grid-cols-3">{roles.slice(0, 5).map(({ label, channelId }) => <RoleCard buffer={buffer} channelId={channelId} descriptors={descriptors} key={`${label}:${channelId}`} label={label} />)}</div>{loop.targetParamId === null && <div className="mb-3"><Alert>设备清单未提供可写目标参数；目标遥测仍可观察，App 不会伪造写入入口。</Alert></div>}<div className="space-y-3">{parameterRecords.map((record) => <TypedParameterControl key={record.paramId} record={record} />)}</div></section>;
}

function RoleCard({ label, channelId, descriptors, buffer }: { label: string; channelId: number | null; descriptors: TelemetryDescriptor[]; buffer: TelemetryRingBuffer }) {
  const descriptor = descriptors.find((item) => item.channelId === channelId);
  const value = channelId === null ? undefined : buffer.latest(channelId)?.value.value;
  return <article className="rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3"><p className="m-0 text-[10px] uppercase tracking-wide text-(--text-muted)">{label}</p><strong className="mt-1 block font-mono text-lg text-(--interactive)">{value === undefined ? "—" : Number(value).toFixed(descriptor?.telemetryType === "f32" ? 3 : 0)}</strong><p className="m-0 mt-1 truncate text-[10px] text-(--text-muted)">{descriptor?.displayName ?? "通道不可用"}{descriptor?.unit ? ` · ${descriptor.unit}` : ""}</p></article>;
}
