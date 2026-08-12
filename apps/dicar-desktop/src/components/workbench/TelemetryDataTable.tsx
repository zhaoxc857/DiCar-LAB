import type { TelemetryDescriptor } from "../../domain/types";
import type { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import { nearestReading, type ActiveCursor } from "../../telemetry/waveformInteraction";
import { formatTelemetry } from "./TelemetryLegend";

type Props = { descriptors: TelemetryDescriptor[]; selectedIds: number[]; buffer: TelemetryRingBuffer; probeTimestampUs: number | null; cursorAUs: number | null; cursorBUs: number | null; activeCursor: ActiveCursor; paused: boolean };

export function TelemetryDataTable({ descriptors, selectedIds, buffer, probeTimestampUs, cursorAUs, cursorBUs, activeCursor, paused }: Props) {
  const targetTimestampUs = probeTimestampUs ?? (activeCursor === "B" ? cursorBUs : cursorAUs);
  const rows = selectedIds.map((channelId) => ({ descriptor: descriptors.find((item) => item.channelId === channelId), latest: targetTimestampUs === null ? buffer.latest(channelId) : nearestReading(buffer, channelId, targetTimestampUs)?.point, a: cursorAUs === null ? null : nearestReading(buffer, channelId, cursorAUs), b: cursorBUs === null ? null : nearestReading(buffer, channelId, cursorBUs), channelId }));
  const timestamp = targetTimestampUs ?? rows.find(({ latest }) => latest)?.latest?.timestampUs;
  const both = cursorAUs !== null && cursorBUs !== null;
  const summary = both
    ? `A ${cursorAUs} µs；B ${cursorBUs} µs；Δt ${Math.abs(cursorBUs - cursorAUs)} µs；${rows.map(({ descriptor, a, b, channelId }) => `${descriptor?.displayName ?? channelId} ${a && b ? `A ${formatTelemetry(a.point.value.value)}，B ${formatTelemetry(b.point.value.value)}，Δ ${formatTelemetry(b.point.value.value - a.point.value.value)}` : "无邻近样本"} ${descriptor?.unit ?? ""}`.trim()).join("；")}`
    : timestamp === undefined ? "暂无遥测样本" : `${probeTimestampUs === null && cursorAUs === null ? "最新" : "游标"} ${timestamp} µs；${rows.map(({ descriptor, latest, channelId }) => `${descriptor?.displayName ?? channelId} ${latest ? formatTelemetry(latest.value.value) : "无邻近样本"} ${descriptor?.unit ?? ""}`.trim()).join("；")}`;
  return <div className="mt-3 border-t border-(--border) pt-3">
    <p aria-label="波形游标读数" aria-live="polite" className="sr-only" role="status">{summary}</p>
    <div className="flex items-center justify-between text-[10px] text-(--text-muted)"><span>{paused ? "波形已暂停，保留最后确认缓冲" : "波形接收中"}</span><span className="font-mono">{timestamp === undefined ? "—" : `${timestamp} µs`}</span></div>
    <div className="mt-2 max-h-32 overflow-auto"><table className="w-full text-left text-[10px]"><thead><tr className="text-(--text-muted)"><th className="py-1">通道</th>{both ? <><th className="py-1">A</th><th className="py-1">B</th><th className="py-1">Δ</th></> : <th className="py-1">值</th>}<th className="py-1">类型</th></tr></thead><tbody>{rows.map(({ descriptor, latest, a, b, channelId }) => <tr className="border-t border-(--border)" key={channelId}><td className="py-1.5">{descriptor?.displayName ?? channelId}</td>{both ? <>{a && b ? <><td className="py-1.5 font-mono">{formatTelemetry(a.point.value.value)} {descriptor?.unit}</td><td className="py-1.5 font-mono">{formatTelemetry(b.point.value.value)} {descriptor?.unit}</td><td className="py-1.5 font-mono">{formatTelemetry(b.point.value.value - a.point.value.value)} {descriptor?.unit}</td></> : <td className="py-1.5 text-(--text-muted)" colSpan={3}>无邻近样本</td>}</> : <td className="py-1.5 font-mono">{latest ? formatTelemetry(latest.value.value) : "—"} {descriptor?.unit}</td>}<td className="py-1.5 font-mono text-(--text-muted)">{latest?.value.kind ?? a?.point.value.kind ?? descriptor?.telemetryType}</td></tr>)}</tbody></table></div>
  </div>;
}
