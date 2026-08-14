import { useEffect, useMemo, useRef, useState } from "react";
import { useDesktopBridge, useRecordingController, useRecordingControllerState } from "../../app/providers";
import type { TelemetryDescriptor } from "../../domain/types";
import { useConnectionStore } from "../../stores/connectionStore";
import { useWorkspaceStore } from "../../stores/workspaceStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import { buildTelemetryWorkgroups, clipWorkgroup, mergeTelemetryWorkgroups, type TelemetryWorkgroup } from "../../telemetry/telemetryWorkgroups";
import { advanceCursor, clampCursorsToBounds, clickCursor, computeChannelRange, type ChannelRange, type WaveformCursorState, type YScaleMode } from "../../telemetry/waveformInteraction";
import { TelemetryDataTable } from "./TelemetryDataTable";
import { TelemetryLegend } from "./TelemetryLegend";
import { TelemetryToolbar } from "./TelemetryToolbar";
import { WaveformCanvas } from "./WaveformCanvas";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Label } from "../ui/label";

export type WaveformSelectionRequest = { requestId: number; label: string; channelIds: number[] };

export function WaveformPanel({ descriptors, selectionRequest = null, profileWorkgroups = [] }: { descriptors: TelemetryDescriptor[]; selectionRequest?: WaveformSelectionRequest | null; profileWorkgroups?: TelemetryWorkgroup[] }) {
  const bridge = useDesktopBridge();
  const recordingController = useRecordingController();
  const recordingState = useRecordingControllerState();
  const snapshot = useConnectionStore((state) => state.snapshot);
  const vehicleProfileId = useVehicleProfileStore((state) => state.selectedProfileId);
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
  const [notice, setNotice] = useState<string | null>(null);
  const [startRecordingOpen, setStartRecordingOpen] = useState(false);
  const [recordingName, setRecordingName] = useState("");
  const [recordingNote, setRecordingNote] = useState("");
  const [recordingBusy, setRecordingBusy] = useState(false);
  const [recordingStartError, setRecordingStartError] = useState<string | null>(null);
  const lastConsumedRequestId = useRef<number | null>(null);
  const paused = snapshot?.paused ?? false;
  const maxChannels = snapshot?.linkBudget?.maxChannels ?? 8;
  const maxSampleRateHz = snapshot?.linkBudget?.maxSampleRateHz ?? 500;
  const selectedDescriptors = useMemo(() => descriptors.filter(({ channelId }) => selectedIds.includes(channelId)), [descriptors, selectedIds]);
  const workgroups = useMemo(() => mergeTelemetryWorkgroups(profileWorkgroups, buildTelemetryWorkgroups(descriptors), descriptors.map(({ channelId }) => channelId)), [descriptors, profileWorkgroups]);
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
  useEffect(() => {
    if (selectionRequest === null || lastConsumedRequestId.current === selectionRequest.requestId) return;
    lastConsumedRequestId.current = selectionRequest.requestId;
    const known = new Set(descriptors.map(({ channelId }) => channelId));
    const available = [...new Set(selectionRequest.channelIds.filter((channelId) => known.has(channelId)))];
    const selected = available.slice(0, maxChannels);
    if (selected.length === 0) {
      setSelectedIds([]);
      setSelectedWorkgroup("custom");
      setYScaleMode("local");
      setFixedRanges({});
      setError(`${selectionRequest.label}没有可用通道`);
      return;
    }
    setSelectedIds(selected);
    setSelectedWorkgroup("custom");
    setYScaleMode("local");
    setFixedRanges({});
    setError(available.length > maxChannels ? `当前链路已保留 ${selected.length} 个通道，省略 ${available.length - selected.length} 个` : null);
  }, [descriptors, maxChannels, selectionRequest]);
  useEffect(() => {
    const firstTimestamps = selectedIds.map((channelId) => buffer.first(channelId)?.timestampUs).filter((timestampUs): timestampUs is number => timestampUs !== undefined);
    const latestTimestamps = selectedIds.map((channelId) => buffer.latest(channelId)?.timestampUs).filter((timestampUs): timestampUs is number => timestampUs !== undefined);
    if (firstTimestamps.length === 0 || latestTimestamps.length === 0) return;
    const firstUs = Math.min(...firstTimestamps);
    const latestUs = Math.max(...latestTimestamps);
    const rolledBeforeStart = (cursors.cursorAUs !== null && cursors.cursorAUs < firstUs) || (cursors.cursorBUs !== null && cursors.cursorBUs < firstUs);
    const result = clampCursorsToBounds(cursors, firstUs, latestUs);
    if (result.clamped.length === 0) return;
    setCursors(result.state);
    setNotice(rolledBeforeStart ? "游标数据已滚出缓冲，已移至最早样本" : "游标超出有效数据，已移至最新样本");
  }, [buffer, cursors, selectedIds, visualRevision]);

  function leaveFixedScale() {
    setYScaleMode("local");
    setFixedRanges({});
  }

  function toggleChannel(channelId: number) {
    setError(null);
    setSelectedWorkgroup("custom");
    if (selectedIds.includes(channelId)) { if (selectedIds.length === 1) { setError("至少保留 1 个通道"); return; } leaveFixedScale(); setSelectedIds((ids) => ids.filter((id) => id !== channelId)); return; }
    if (selectedIds.length >= maxChannels) { setError(maxChannels === 8 ? "最多同时显示 8 个通道" : `当前链路最多同时显示 ${maxChannels} 个通道`); return; }
    leaveFixedScale();
    setSelectedIds((ids) => [...ids, channelId]);
  }
  function selectWorkgroup(id: string) {
    const group = workgroups.find((item) => item.id === id);
    if (!group) return;
    const clipped = clipWorkgroup(group, maxChannels);
    leaveFixedScale();
    setSelectedIds(clipped.channelIds);
    setSelectedWorkgroup(id);
    setError(clipped.omittedCount > 0 ? `当前链路已保留 ${clipped.channelIds.length} 个通道，省略 ${clipped.omittedCount} 个` : null);
  }
  async function applySubscription() {
    if (recordingState.active !== null) {
      try {
        await recordingController.stop("subscriptionChanged");
      } catch {
        return;
      }
    }
    const result = await bridge.setTelemetrySubscription({ channelIds: selectedIds, sampleRateHz });
    if (result.status === "failed") setError(result.message); else setError(null);
  }
  async function togglePause() {
    if (!paused && recordingState.active !== null) {
      try {
        await recordingController.stop("paused");
      } catch {
        return;
      }
    }
    const result = await bridge.setPaused(!paused);
    if (result.status === "failed") setError(result.message);
  }
  async function startRecording() {
    if (snapshot !== null) recordingController.setSnapshot(snapshot);
    setRecordingBusy(true);
    setRecordingStartError(null);
    try {
      await recordingController.start({ name: recordingName, note: recordingNote, vehicleProfileId });
      setStartRecordingOpen(false);
      setRecordingName("");
      setRecordingNote("");
    } catch (reason) {
      setRecordingStartError(reason instanceof Error ? reason.message : "无法开始波形记录");
    } finally {
      setRecordingBusy(false);
    }
  }
  async function stopRecording() {
    setRecordingBusy(true);
    try {
      await recordingController.stop("manual");
    } catch {
      setError("波形记录封存失败");
    } finally {
      setRecordingBusy(false);
    }
  }
  async function addMarker() { const active = cursors.activeCursor === "B" ? cursors.cursorBUs : cursors.cursorAUs; const timestampUs = active ?? probeTimestampUs ?? (selectedIds[0] === undefined ? undefined : buffer.latest(selectedIds[0])?.timestampUs); if (timestampUs === undefined || timestampUs === null) { setError("尚无可标记的遥测时刻"); return; } const result = await bridge.addMarker(`T+${Math.round(timestampUs)} µs`); if (result.status === "failed") setError(result.message); }
  function lockCursor(timestampUs: number) { setNotice(null); setCursors((state) => clickCursor(state, Math.round(timestampUs))); }
  function clearCursors() { setCursors({ cursorAUs: null, cursorBUs: null, activeCursor: "A" }); setProbeTimestampUs(null); setNotice(null); }
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
    else if (event.key === "Enter" && selectedIds[0] !== undefined) {
      event.preventDefault();
      setCursors((state) => {
        const activeTimestamp = state.activeCursor === "B" ? state.cursorBUs : state.cursorAUs;
        const timestampUs = activeTimestamp ?? buffer.latest(selectedIds[0])?.timestampUs;
        return timestampUs === undefined ? state : clickCursor(state, timestampUs);
      });
    }
    else if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && selectedIds[0] !== undefined) { event.preventDefault(); setCursors((state) => advanceCursor(buffer, selectedIds[0], state, event.key === "ArrowLeft" ? -1 : 1, event.shiftKey ? 10 : 1)); }
    else if (event.key.toLocaleLowerCase() === "a" && cursors.cursorAUs !== null) { event.preventDefault(); setCursors((state) => ({ ...state, activeCursor: "A" })); }
    else if (event.key.toLocaleLowerCase() === "b" && cursors.cursorBUs !== null) { event.preventDefault(); setCursors((state) => ({ ...state, activeCursor: "B" })); }
    else if (event.key === "Escape") { event.preventDefault(); if (probeTimestampUs !== null) setProbeTimestampUs(null); else clearCursors(); }
    else if (event.key.toLocaleLowerCase() === "m") { event.preventDefault(); void addMarker(); }
  }

  return <section className="min-w-0 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3">
    <header className="flex items-start justify-between gap-3"><div><h2 className="m-0 text-sm">实时波形</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">有界 60 秒缓冲 · 像素极值降采样 · Canvas ≤30 Hz</p></div><span className="rounded border border-(--success) px-2 py-1 text-[10px] text-(--success)">{selectedIds.length}/{maxChannels} 通道</span></header>
    <TelemetryToolbar descriptors={descriptors} error={error ?? recordingState.error} hasCursors={cursors.cursorAUs !== null || cursors.cursorBUs !== null} linkReason={snapshot?.linkBudget?.reason ?? null} maxChannels={maxChannels} maxSampleRateHz={maxSampleRateHz} onApply={() => void applySubscription()} onClearCursors={clearCursors} onMarker={() => void addMarker()} onPause={() => void togglePause()} onResetFixedRanges={resetFixedRanges} onSampleRate={setSampleRateHz} onStartRecording={() => { setRecordingStartError(null); setStartRecordingOpen(true); }} onStopRecording={() => void stopRecording()} onToggleChannel={toggleChannel} onWindow={setWindowSeconds} onWorkgroup={selectWorkgroup} onYScaleMode={changeScaleMode} paused={paused} recordingActive={recordingState.active !== null} recordingName={recordingState.active?.name ?? null} sampleRateHz={sampleRateHz} selectedIds={selectedIds} selectedWorkgroup={selectedWorkgroup} windowSeconds={windowSeconds} workgroups={workgroups} yScaleMode={yScaleMode} />
    {recordingState.notice !== null && <p aria-live="polite" className="m-0 mt-2 text-[10px] text-(--success)">{recordingState.notice}</p>}
    <div aria-label="实时波形交互区" className="mt-3 overflow-hidden rounded border border-(--border) bg-(--background) focus-visible:outline" onKeyDown={onKeyDown} role="region" tabIndex={0}><WaveformCanvas buffer={buffer} cursorAUs={cursors.cursorAUs} cursorBUs={cursors.cursorBUs} descriptors={selectedDescriptors} fixedRanges={fixedRanges} onLockCursor={lockCursor} onProbe={setProbeTimestampUs} paused={paused} probeTimestampUs={probeTimestampUs} selectedIds={selectedIds} visualRevision={visualRevision} windowSeconds={windowSeconds} yScaleMode={yScaleMode} /></div>
    {notice ? <p aria-live="polite" className="m-0 mt-2 text-[10px] text-(--warning)">{notice}</p> : null}
    <TelemetryLegend buffer={buffer} descriptors={descriptors} selectedIds={selectedIds} targetTimestampUs={targetTimestampUs} />
    <TelemetryDataTable activeCursor={cursors.activeCursor} buffer={buffer} cursorAUs={cursors.cursorAUs} cursorBUs={cursors.cursorBUs} descriptors={descriptors} paused={paused} probeTimestampUs={probeTimestampUs} selectedIds={selectedIds} />
    {startRecordingOpen && <div className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4"><section aria-labelledby="start-recording-title" aria-modal="true" className="w-full max-w-md rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-4 shadow-2xl" role="dialog"><h3 className="m-0 text-base" id="start-recording-title">开始波形记录</h3><p className="m-0 mt-1 text-xs text-(--text-muted)">保存原始遥测批次，最长 5 分钟。暂停、断线或订阅变化会自动封存。</p><div className="mt-4 space-y-3"><div><Label htmlFor="recording-name">记录名称</Label><Input autoFocus id="recording-name" maxLength={64} onChange={(event) => setRecordingName(event.currentTarget.value)} value={recordingName} /></div><div><Label htmlFor="recording-note">记录备注</Label><textarea className="min-h-20 w-full rounded-[var(--radius)] border border-(--border) bg-(--background) p-3 text-sm text-(--text)" id="recording-note" maxLength={256} onChange={(event) => setRecordingNote(event.currentTarget.value)} value={recordingNote} /></div></div>{recordingStartError !== null && <p aria-live="assertive" className="m-0 mt-3 text-xs text-(--danger)">{recordingStartError}</p>}<div className="mt-4 flex justify-end gap-2"><Button disabled={recordingBusy} onClick={() => setStartRecordingOpen(false)} size="sm" variant="secondary">取消</Button><Button disabled={recordingBusy} onClick={() => void startRecording()} size="sm">确认开始</Button></div></section></div>}
  </section>;
}
