import { useEffect, useMemo, useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import type { TelemetryDescriptor } from "../../domain/types";
import { useConnectionStore } from "../../stores/connectionStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { buildTelemetryWorkgroups, clipWorkgroup } from "../../telemetry/telemetryWorkgroups";
import { advanceCursor, clickCursor, computeChannelRange, type ChannelRange, type WaveformCursorState, type YScaleMode } from "../../telemetry/waveformInteraction";
import { TelemetryDataTable } from "./TelemetryDataTable";
import { TelemetryLegend } from "./TelemetryLegend";
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
  const [probeTimestampUs, setProbeTimestampUs] = useState<number | null>(null);
  const [cursors, setCursors] = useState<WaveformCursorState>({ cursorAUs: null, cursorBUs: null, activeCursor: "A" });
  const [selectedWorkgroup, setSelectedWorkgroup] = useState("all");
  const [yScaleMode, setYScaleMode] = useState<YScaleMode>("local");
  const [fixedRanges, setFixedRanges] = useState<Record<number, ChannelRange>>({});
  const [error, setError] = useState<string | null>(null);
  const paused = snapshot?.paused ?? false;
  const maxChannels = snapshot?.linkBudget?.maxChannels ?? 8;
  const maxSampleRateHz = snapshot?.linkBudget?.maxSampleRateHz ?? 500;
  const selectedDescriptors = useMemo(() => descriptors.filter(({ channelId }) => selectedIds.includes(channelId)), [descriptors, selectedIds]);
  const workgroups = useMemo(() => buildTelemetryWorkgroups(descriptors), [descriptors]);
  const targetTimestampUs = probeTimestampUs ?? (cursors.activeCursor === "B" ? cursors.cursorBUs : cursors.cursorAUs);

  useEffect(() => {
    if (descriptors.length === 0) return;
    const known = new Set(descriptors.map(({ channelId }) => channelId));
    setSelectedIds((current) => {
      const retained = current.filter((channelId) => known.has(channelId));
      const next = (retained.length > 0 ? retained : descriptors.map(({ channelId }) => channelId)).slice(0, maxChannels);
      return next.length === current.length && next.every((channelId, index) => channelId === current[index]) ? current : next;
    });
  }, [descriptors, maxChannels]);
  useEffect(() => { setSampleRateHz((current) => Math.min(current, maxSampleRateHz)); }, [maxSampleRateHz]);

  function toggleChannel(channelId: number) {
    setError(null);
    setSelectedWorkgroup("custom");
    if (selectedIds.includes(channelId)) { if (selectedIds.length === 1) { setError("至少保留 1 个通道"); return; } setSelectedIds((ids) => ids.filter((id) => id !== channelId)); return; }
    if (selectedIds.length >= maxChannels) { setError(maxChannels === 8 ? "最多同时显示 8 个通道" : `当前链路最多同时显示 ${maxChannels} 个通道`); return; }
    setSelectedIds((ids) => [...ids, channelId]);
  }
  function selectWorkgroup(id: string) {
    const group = workgroups.find((item) => item.id === id);
    if (!group) return;
    const clipped = clipWorkgroup(group, maxChannels);
    setSelectedIds(clipped.channelIds);
    setSelectedWorkgroup(id);
    setError(clipped.omittedCount > 0 ? `当前链路已保留 ${clipped.channelIds.length} 个通道，省略 ${clipped.omittedCount} 个` : null);
  }
  async function applySubscription() { const result = await bridge.setTelemetrySubscription({ channelIds: selectedIds, sampleRateHz }); if (result.status === "failed") setError(result.message); else setError(null); }
  async function togglePause() { const result = await bridge.setPaused(!paused); if (result.status === "failed") setError(result.message); }
  async function addMarker() { const active = cursors.activeCursor === "B" ? cursors.cursorBUs : cursors.cursorAUs; const timestampUs = active ?? probeTimestampUs ?? (selectedIds[0] === undefined ? undefined : buffer.latest(selectedIds[0])?.timestampUs); if (timestampUs === undefined || timestampUs === null) { setError("尚无可标记的遥测时刻"); return; } const result = await bridge.addMarker(`T+${Math.round(timestampUs)} µs`); if (result.status === "failed") setError(result.message); }
  function lockCursor(timestampUs: number) { setCursors((state) => clickCursor(state, Math.round(timestampUs))); }
  function clearCursors() { setCursors({ cursorAUs: null, cursorBUs: null, activeCursor: "A" }); setProbeTimestampUs(null); }
  function changeScaleMode(mode: YScaleMode) {
    setYScaleMode(mode);
    if (mode === "fixed") resetFixedRanges();
  }
  function resetFixedRanges() {
    const ranges: Record<number, ChannelRange> = {};
    const latestUs = Math.max(0, ...selectedIds.map((channelId) => buffer.latest(channelId)?.timestampUs ?? 0));
    const firstUs = Math.max(0, latestUs - windowSeconds * 1_000_000);
    for (const channelId of selectedIds) {
      const range = computeChannelRange(buffer.snapshot(channelId, firstUs).map((point) => point.value.value));
      if (range) ranges[channelId] = range;
    }
    setFixedRanges(ranges);
  }

  function onKeyDown(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key === " ") { event.preventDefault(); void togglePause(); }
    else if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && selectedIds[0] !== undefined) { event.preventDefault(); setCursors((state) => advanceCursor(buffer, selectedIds[0], state, event.key === "ArrowLeft" ? -1 : 1, event.shiftKey ? 10 : 1)); }
    else if (event.key.toLocaleLowerCase() === "a" && cursors.cursorAUs !== null) { event.preventDefault(); setCursors((state) => ({ ...state, activeCursor: "A" })); }
    else if (event.key.toLocaleLowerCase() === "b" && cursors.cursorBUs !== null) { event.preventDefault(); setCursors((state) => ({ ...state, activeCursor: "B" })); }
    else if (event.key === "Escape") { event.preventDefault(); if (probeTimestampUs !== null) setProbeTimestampUs(null); else clearCursors(); }
    else if (event.key.toLocaleLowerCase() === "m") { event.preventDefault(); void addMarker(); }
  }

  return <section className="min-w-0 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3">
    <header className="flex items-start justify-between gap-3"><div><h2 className="m-0 text-sm">实时波形</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">有界 60 秒缓冲 · 像素极值降采样 · Canvas ≤30 Hz</p></div><span className="rounded border border-(--success) px-2 py-1 text-[10px] text-(--success)">{selectedIds.length}/{maxChannels} 通道</span></header>
    <TelemetryToolbar descriptors={descriptors} error={error} hasCursors={cursors.cursorAUs !== null || cursors.cursorBUs !== null} linkReason={snapshot?.linkBudget?.reason ?? null} maxChannels={maxChannels} maxSampleRateHz={maxSampleRateHz} onApply={() => void applySubscription()} onClearCursors={clearCursors} onMarker={() => void addMarker()} onPause={() => void togglePause()} onResetFixedRanges={resetFixedRanges} onSampleRate={setSampleRateHz} onToggleChannel={toggleChannel} onWindow={setWindowSeconds} onWorkgroup={selectWorkgroup} onYScaleMode={changeScaleMode} paused={paused} sampleRateHz={sampleRateHz} selectedIds={selectedIds} selectedWorkgroup={selectedWorkgroup} windowSeconds={windowSeconds} workgroups={workgroups} yScaleMode={yScaleMode} />
    <div aria-label="实时波形交互区" className="mt-3 overflow-hidden rounded border border-(--border) bg-(--background) focus-visible:outline" onKeyDown={onKeyDown} role="region" tabIndex={0}><WaveformCanvas buffer={buffer} cursorAUs={cursors.cursorAUs} cursorBUs={cursors.cursorBUs} descriptors={selectedDescriptors} fixedRanges={fixedRanges} onLockCursor={lockCursor} onProbe={setProbeTimestampUs} paused={paused} probeTimestampUs={probeTimestampUs} selectedIds={selectedIds} visualRevision={visualRevision} windowSeconds={windowSeconds} yScaleMode={yScaleMode} /></div>
    <TelemetryLegend buffer={buffer} descriptors={descriptors} selectedIds={selectedIds} targetTimestampUs={targetTimestampUs} />
    <TelemetryDataTable activeCursor={cursors.activeCursor} buffer={buffer} cursorAUs={cursors.cursorAUs} cursorBUs={cursors.cursorBUs} descriptors={descriptors} paused={paused} probeTimestampUs={probeTimestampUs} selectedIds={selectedIds} />
  </section>;
}
