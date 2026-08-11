import type { TelemetryDescriptor } from "../../domain/types";
import type { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import { channelStyle } from "../../telemetry/channelStyles";

export function TelemetryLegend({ descriptors, selectedIds, buffer, cursorIndex }: { descriptors: TelemetryDescriptor[]; selectedIds: number[]; buffer: TelemetryRingBuffer; cursorIndex: number }) {
  return <div aria-label="波形图例" className="mt-3 grid gap-1.5 sm:grid-cols-2">{selectedIds.map((channelId, slot) => {
    const descriptor = descriptors.find((item) => item.channelId === channelId);
    const point = cursorPoint(buffer, channelId, cursorIndex);
    const style = channelStyle(slot);
    return <div className="flex min-w-0 items-center gap-2 text-[10px]" key={channelId}><span className="h-0 w-5 shrink-0 border-t-2" style={{ borderColor: style.color, borderStyle: style.dash.length ? "dashed" : "solid" }} /><span className="truncate">{descriptor?.displayName ?? `通道 ${channelId}`}</span><span className="ml-auto font-mono text-(--text-muted)">{point ? formatTelemetry(point.value.value) : "—"} {descriptor?.unit}</span></div>;
  })}</div>;
}

export function cursorPoint(buffer: TelemetryRingBuffer, channelId: number, cursorIndex: number) {
  const length = buffer.length(channelId);
  if (length === 0) return undefined;
  return buffer.at(channelId, Math.max(0, Math.min(cursorIndex < 0 ? length - 1 : cursorIndex, length - 1)));
}

export function formatTelemetry(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(3).replace(/0+$/, "").replace(/\.$/, "");
}
