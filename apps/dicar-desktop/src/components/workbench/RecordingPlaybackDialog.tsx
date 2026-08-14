import { CaretLeft, CaretRight, Pause, Play, X } from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { useRecordingController } from "../../app/providers";
import { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import type { TelemetryRecordingDocument } from "../../telemetry/recordings";
import { Button } from "../ui/button";
import { Select } from "../ui/select";
import { TelemetryDataTable } from "./TelemetryDataTable";
import { TelemetryLegend } from "./TelemetryLegend";
import { WaveformCanvas } from "./WaveformCanvas";

type PlaybackSession = {
  document: TelemetryRecordingDocument;
  buffer: TelemetryRingBuffer;
  timestamps: number[];
  firstUs: number;
  lastUs: number;
};

type Props = {
  open: boolean;
  recordingId: string | null;
  onClose: () => void;
};

const PLAYBACK_SPEEDS = [0.25, 0.5, 1, 2, 4] as const;

export function RecordingPlaybackDialog({ open, recordingId, onClose }: Props) {
  const controller = useRecordingController();
  const [session, setSession] = useState<PlaybackSession | null>(null);
  const [currentUs, setCurrentUs] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState<number>(1);
  const [probeTimestampUs, setProbeTimestampUs] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const previousFrameMs = useRef<number | null>(null);

  useEffect(() => {
    if (!open || recordingId === null) return;
    const release = controller.protect(recordingId);
    let cancelled = false;
    setSession(null);
    setPlaying(false);
    setError(null);
    void controller.getDocument(recordingId).then((document) => {
      if (cancelled) return;
      if (document === null) throw new Error("记录不存在或尚未封存");
      const built = buildPlaybackSession(document);
      setSession(built);
      setCurrentUs(built.firstUs);
    }).catch((reason) => {
      if (!cancelled) setError(reason instanceof Error ? reason.message : "无法读取回放记录");
    });
    return () => {
      cancelled = true;
      release();
    };
  }, [controller, open, recordingId]);

  useEffect(() => {
    if (!playing || session === null) {
      previousFrameMs.current = null;
      return;
    }
    let frameId = 0;
    const tick = (frameMs: number) => {
      const previous = previousFrameMs.current;
      previousFrameMs.current = frameMs;
      if (previous !== null) {
        setCurrentUs((current) => Math.min(session.lastUs, current + (frameMs - previous) * 1_000 * speed));
      }
      frameId = requestAnimationFrame(tick);
    };
    frameId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frameId);
  }, [playing, session, speed]);

  useEffect(() => {
    if (playing && session !== null && currentUs >= session.lastUs) setPlaying(false);
  }, [currentUs, playing, session]);

  const selectedIds = useMemo(
    () => session?.document.metadata.channelDescriptors.map(({ channelId }) => channelId) ?? [],
    [session],
  );

  if (!open || recordingId === null) return null;

  function step(direction: -1 | 1): void {
    if (session === null) return;
    setPlaying(false);
    const next = direction > 0
      ? session.timestamps.find((timestampUs) => timestampUs > currentUs) ?? session.lastUs
      : [...session.timestamps].reverse().find((timestampUs) => timestampUs < currentUs) ?? session.firstUs;
    setCurrentUs(next);
  }

  return <div className="fixed inset-0 z-[60] grid place-items-center bg-black/75 p-4" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <section aria-labelledby="recording-playback-title" aria-modal="true" className="max-h-[90vh] w-full max-w-5xl overflow-auto rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) shadow-2xl" role="dialog">
      <header className="flex items-start justify-between gap-3 border-b border-(--border) p-4"><div><h2 className="m-0 text-base" id="recording-playback-title">{session === null ? "波形回放" : `回放 · ${session.document.metadata.name}`}</h2><p className="m-0 mt-1 text-xs text-(--text-muted)">独立只读缓冲；不会暂停设备、替换实时波形或发送 Bridge 命令。</p></div><Button aria-label="关闭波形回放" onClick={onClose} size="sm" variant="secondary"><X size={15} /></Button></header>
      <div className="p-4">{error !== null ? <p aria-live="assertive" className="m-0 text-sm text-(--danger)">{error}</p> : session === null ? <p className="m-0 text-sm text-(--text-muted)">正在载入回放…</p> : <>
        <div className="mb-3 flex flex-wrap items-center gap-2"><Button aria-label={playing ? "暂停回放" : "播放回放"} onClick={() => setPlaying((value) => !value)} size="sm">{playing ? <Pause size={14} /> : <Play size={14} />}{playing ? "暂停" : "播放"}</Button><Button aria-label="上一采样时刻" onClick={() => step(-1)} size="sm" variant="secondary"><CaretLeft size={14} />单步</Button><Button aria-label="下一采样时刻" onClick={() => step(1)} size="sm" variant="secondary"><CaretRight size={14} />单步</Button><label className="text-[10px] text-(--text-muted)">速度<Select aria-label="回放速度" className="ml-2 inline-block h-8 w-24 text-xs" onChange={(event) => setSpeed(Number(event.currentTarget.value))} value={speed}>{PLAYBACK_SPEEDS.map((value) => <option key={value} value={value}>{value}×</option>)}</Select></label><output aria-live="polite" className="ml-auto font-mono text-xs text-(--text-muted)">{formatSeconds(currentUs)} / {formatSeconds(session.lastUs)}</output></div>
        <input aria-label="回放进度" className="mb-3 w-full accent-(--interactive)" max={session.lastUs} min={session.firstUs} onChange={(event) => { setPlaying(false); setCurrentUs(Number(event.currentTarget.value)); }} step={1} type="range" value={currentUs} />
        <div className="overflow-hidden rounded border border-(--border) bg-(--background)"><WaveformCanvas ariaLabel="回放波形" buffer={session.buffer} cursorAUs={null} cursorBUs={null} descriptors={session.document.metadata.channelDescriptors} fixedRanges={{}} onLockCursor={(timestampUs) => { setPlaying(false); setCurrentUs(Math.max(session.firstUs, Math.min(session.lastUs, Math.round(timestampUs)))); }} onProbe={setProbeTimestampUs} paused={!playing} probeTimestampUs={probeTimestampUs} selectedIds={selectedIds} viewportEndUs={currentUs} visualRevision={Math.round(currentUs)} windowSeconds={10} yScaleMode="local" /></div>
        <TelemetryLegend buffer={session.buffer} descriptors={session.document.metadata.channelDescriptors} selectedIds={selectedIds} targetTimestampUs={probeTimestampUs ?? currentUs} />
        <TelemetryDataTable activeCursor="A" buffer={session.buffer} cursorAUs={null} cursorBUs={null} descriptors={session.document.metadata.channelDescriptors} paused probeTimestampUs={probeTimestampUs ?? currentUs} selectedIds={selectedIds} />
      </>}</div>
    </section>
  </div>;
}

function buildPlaybackSession(document: TelemetryRecordingDocument): PlaybackSession {
  const pointCountPerChannel = Math.max(1, document.metadata.stats.pointCount);
  const buffer = new TelemetryRingBuffer(Math.max(1, document.metadata.channelDescriptors.length), pointCountPerChannel);
  const timestamps = new Set<number>();
  for (const chunk of document.chunks) {
    for (const batch of chunk.batches) {
      buffer.append(batch.points);
      for (const point of batch.points) timestamps.add(point.timestampUs);
    }
  }
  const ordered = [...timestamps].sort((left, right) => left - right);
  const firstUs = ordered[0] ?? 0;
  const lastUs = ordered.at(-1) ?? firstUs;
  return { document, buffer, timestamps: ordered, firstUs, lastUs };
}

function formatSeconds(timestampUs: number): string {
  return `${(timestampUs / 1_000_000).toFixed(3)} s`;
}
