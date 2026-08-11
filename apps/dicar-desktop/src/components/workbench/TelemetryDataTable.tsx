import type { TelemetryDescriptor } from "../../domain/types";
import type { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import { cursorPoint, formatTelemetry } from "./TelemetryLegend";

export function TelemetryDataTable({ descriptors, selectedIds, buffer, cursorIndex, paused }: { descriptors: TelemetryDescriptor[]; selectedIds: number[]; buffer: TelemetryRingBuffer; cursorIndex: number; paused: boolean }) {
  const rows = selectedIds.map((channelId) => ({ descriptor: descriptors.find((item) => item.channelId === channelId), point: cursorPoint(buffer, channelId, cursorIndex), channelId }));
  const timestamp = rows.find(({ point }) => point)?.point?.timestampUs;
  const summary = timestamp === undefined ? "暂无遥测样本" : `游标 ${timestamp} µs；${rows.map(({ descriptor, point, channelId }) => `${descriptor?.displayName ?? channelId} ${point ? formatTelemetry(point.value.value) : "—"} ${descriptor?.unit ?? ""}`.trim()).join("；")}`;
  return <div className="mt-3 border-t border-(--border) pt-3">
    <p aria-label="波形游标读数" aria-live="polite" className="sr-only" role="status">{summary}</p>
    <div className="flex items-center justify-between text-[10px] text-(--text-muted)"><span>{paused ? "波形已暂停，保留最后确认缓冲" : "波形接收中"}</span><span className="font-mono">{timestamp === undefined ? "—" : `${timestamp} µs`}</span></div>
    <div className="mt-2 max-h-32 overflow-auto"><table className="w-full text-left text-[10px]"><thead><tr className="text-(--text-muted)"><th className="py-1">通道</th><th className="py-1">值</th><th className="py-1">类型</th></tr></thead><tbody>{rows.map(({ descriptor, point, channelId }) => <tr className="border-t border-(--border)" key={channelId}><td className="py-1.5">{descriptor?.displayName ?? channelId}</td><td className="py-1.5 font-mono">{point ? formatTelemetry(point.value.value) : "—"} {descriptor?.unit}</td><td className="py-1.5 font-mono text-(--text-muted)">{point?.value.kind ?? descriptor?.telemetryType}</td></tr>)}</tbody></table></div>
  </div>;
}
