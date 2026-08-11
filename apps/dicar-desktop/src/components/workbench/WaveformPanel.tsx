import { useEffect, useMemo, useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import type { TelemetryDescriptor } from "../../domain/types";
import { useConnectionStore } from "../../stores/connectionStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { TelemetryDataTable } from "./TelemetryDataTable";
import { TelemetryLegend, cursorPoint } from "./TelemetryLegend";
import { TelemetryToolbar } from "./TelemetryToolbar";
import { WaveformCanvas } from "./WaveformCanvas";

export function WaveformPanel({ descriptors }: { descriptors: TelemetryDescriptor[] }) {
  const bridge = useDesktopBridge();
  const snapshot = useConnectionStore((state) => state.snapshot);
  const buffer = useWorkspaceStore((state) => state.buffer);
  const visualRevision = useWorkspaceStore((state) => state.visualRevision);
  const [selectedIds, setSelectedIds] = useState(() => descriptors.slice(0, 8).map(({ channelId }) => channelId));
  const [sampleRateHz, setSampleRateHz] = useState(500);
  const [windowSeconds, setWindowSeconds] = useState(10);
  const [cursorIndex, setCursorIndex] = useState(-1);
  const [error, setError] = useState<string | null>(null);
  const paused = snapshot?.paused ?? false;
  const firstChannelLength = selectedIds[0] === undefined ? 0 : buffer.length(selectedIds[0]);
  const selectedDescriptors = useMemo(() => descriptors.filter(({ channelId }) => selectedIds.includes(channelId)), [descriptors, selectedIds]);

  useEffect(() => {
    if (descriptors.length === 0) return;
    const known = new Set(descriptors.map(({ channelId }) => channelId));
    setSelectedIds((current) => {
      const retained = current.filter((channelId) => known.has(channelId));
      const next = retained.length > 0 ? retained : descriptors.slice(0, 8).map(({ channelId }) => channelId);
      return next.length === current.length && next.every((channelId, index) => channelId === current[index]) ? current : next;
    });
  }, [descriptors]);
  useEffect(() => { if (firstChannelLength > 0) setCursorIndex((index) => index < 0 ? firstChannelLength - 1 : Math.min(index, firstChannelLength - 1)); }, [firstChannelLength, visualRevision]);

  function toggleChannel(channelId: number) {
    setError(null);
    if (selectedIds.includes(channelId)) { if (selectedIds.length === 1) { setError("至少保留 1 个通道"); return; } setSelectedIds((ids) => ids.filter((id) => id !== channelId)); return; }
    if (selectedIds.length >= 8) { setError("最多同时显示 8 个通道"); return; }
    setSelectedIds((ids) => [...ids, channelId]);
  }
  async function applySubscription() { const result = await bridge.setTelemetrySubscription({ channelIds: selectedIds, sampleRateHz }); if (result.status === "failed") setError(result.message); else setError(null); }
  async function togglePause() { const result = await bridge.setPaused(!paused); if (result.status === "failed") setError(result.message); }
  async function addMarker() { const point = selectedIds[0] === undefined ? undefined : cursorPoint(buffer, selectedIds[0], cursorIndex); if (!point) { setError("尚无可标记的遥测时刻"); return; } const result = await bridge.addMarker(`T+${point.timestampUs} µs`); if (result.status === "failed") setError(result.message); }

  function onKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key === " ") { event.preventDefault(); void togglePause(); }
    else if (event.key === "ArrowLeft") { event.preventDefault(); setCursorIndex((index) => Math.max(0, (index < 0 ? firstChannelLength : index) - 1)); }
    else if (event.key === "ArrowRight") { event.preventDefault(); setCursorIndex((index) => Math.min(Math.max(0, firstChannelLength - 1), index + 1)); }
    else if (event.key.toLocaleLowerCase() === "m") { event.preventDefault(); void addMarker(); }
  }

  return <section className="min-w-0 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3">
    <header className="flex items-start justify-between gap-3"><div><h2 className="m-0 text-sm">实时波形</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">有界 60 秒缓冲 · 像素极值降采样 · Canvas ≤30 Hz</p></div><span className="rounded border border-(--success) px-2 py-1 text-[10px] text-(--success)">{selectedIds.length}/8 通道</span></header>
    <TelemetryToolbar descriptors={descriptors} error={error} onApply={() => void applySubscription()} onMarker={() => void addMarker()} onPause={() => void togglePause()} onSampleRate={setSampleRateHz} onToggleChannel={toggleChannel} onWindow={setWindowSeconds} paused={paused} sampleRateHz={sampleRateHz} selectedIds={selectedIds} windowSeconds={windowSeconds} />
    <div aria-label="实时波形交互区" className="mt-3 overflow-hidden rounded border border-(--border) bg-(--background) focus-visible:outline" onKeyDown={onKeyDown} role="region" tabIndex={0}><WaveformCanvas buffer={buffer} cursorIndex={cursorIndex} descriptors={selectedDescriptors} paused={paused} selectedIds={selectedIds} visualRevision={visualRevision} windowSeconds={windowSeconds} /></div>
    <TelemetryLegend buffer={buffer} cursorIndex={cursorIndex} descriptors={descriptors} selectedIds={selectedIds} />
    <TelemetryDataTable buffer={buffer} cursorIndex={cursorIndex} descriptors={descriptors} paused={paused} selectedIds={selectedIds} />
  </section>;
}
