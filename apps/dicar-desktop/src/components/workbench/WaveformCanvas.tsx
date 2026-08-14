import { useEffect, useRef, useState } from "react";
import type { TelemetryDescriptor } from "../../domain/types";
import { channelStyle } from "../../telemetry/channelStyles";
import { minMaxBuckets } from "../../telemetry/downsample";
import type { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import { computeChannelRange, timestampForX, xForTimestamp, type ChannelRange, type YScaleMode } from "../../telemetry/waveformInteraction";

type Size = { width: number; height: number };

type WaveformCanvasProps = { buffer: TelemetryRingBuffer; descriptors: TelemetryDescriptor[]; selectedIds: number[]; windowSeconds: number; probeTimestampUs: number | null; cursorAUs: number | null; cursorBUs: number | null; visualRevision: number; paused: boolean; yScaleMode: YScaleMode; fixedRanges: Record<number, ChannelRange>; onProbe: (timestampUs: number | null) => void; onLockCursor: (timestampUs: number) => void; viewportEndUs?: number | null; ariaLabel?: string };

export function WaveformCanvas({ buffer, descriptors, selectedIds, windowSeconds, probeTimestampUs, cursorAUs, cursorBUs, visualRevision, paused, yScaleMode, fixedRanges, onProbe, onLockCursor, viewportEndUs = null, ariaLabel = "实时波形" }: WaveformCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState<Size>({ width: 0, height: 0 });

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(([entry]) => setSize({ width: Math.max(1, Math.floor(entry.contentRect.width)), height: Math.max(1, Math.floor(entry.contentRect.height)) }));
    observer.observe(canvas);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || size.width === 0 || size.height === 0) return;
    const frame = requestAnimationFrame(() => draw(canvas, size, buffer, descriptors, selectedIds, windowSeconds, probeTimestampUs, cursorAUs, cursorBUs, yScaleMode, fixedRanges, viewportEndUs));
    return () => cancelAnimationFrame(frame);
  }, [buffer, cursorAUs, cursorBUs, descriptors, fixedRanges, paused, probeTimestampUs, selectedIds, size, viewportEndUs, visualRevision, windowSeconds, yScaleMode]);

  function pointerTimestamp(event: React.MouseEvent<HTMLCanvasElement>): number | null {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const bounds = visibleBounds(buffer, selectedIds, windowSeconds, viewportEndUs);
    if (bounds === null) return null;
    const rect = canvas.getBoundingClientRect();
    return timestampForX(event.clientX - rect.left, rect.width, bounds.firstUs, bounds.latestUs);
  }

  return <canvas aria-label={ariaLabel} className="block h-48 w-full 2xl:h-64" onClick={(event) => { const timestamp = pointerTimestamp(event); if (timestamp !== null) onLockCursor(timestamp); }} onMouseLeave={() => onProbe(null)} onMouseMove={(event) => onProbe(pointerTimestamp(event))} ref={canvasRef} role="img" />;
}

function draw(canvas: HTMLCanvasElement, size: Size, buffer: TelemetryRingBuffer, descriptors: TelemetryDescriptor[], selectedIds: number[], windowSeconds: number, probeTimestampUs: number | null, cursorAUs: number | null, cursorBUs: number | null, yScaleMode: YScaleMode, fixedRanges: Record<number, ChannelRange>, viewportEndUs: number | null) {
  const dpr = Math.max(1, window.devicePixelRatio || 1);
  canvas.width = Math.max(1, Math.floor(size.width * dpr));
  canvas.height = Math.max(1, Math.floor(size.height * dpr));
  const context = canvas.getContext("2d");
  if (!context) return;
  context.setTransform(dpr, 0, 0, dpr, 0, 0);
  context.clearRect(0, 0, size.width, size.height);
  context.fillStyle = "#07111b";
  context.fillRect(0, 0, size.width, size.height);
  context.strokeStyle = "rgba(148,163,184,.1)";
  context.lineWidth = 1;
  for (let x = 0; x < size.width; x += 48) { context.beginPath(); context.moveTo(x, 0); context.lineTo(x, size.height); context.stroke(); }
  for (let y = 0; y < size.height; y += 32) { context.beginPath(); context.moveTo(0, y); context.lineTo(size.width, y); context.stroke(); }
  const bounds = visibleBounds(buffer, selectedIds, windowSeconds, viewportEndUs);
  const latestUs = bounds?.latestUs ?? 0;
  const firstUs = bounds?.firstUs ?? 0;
  const bandHeight = Math.max(24, (size.height - 18) / Math.max(1, selectedIds.length));
  selectedIds.forEach((channelId, slot) => {
    const points = buffer.snapshot(channelId, firstUs).filter((point) => point.timestampUs <= latestUs);
    if (points.length === 0) return;
    const rangePoints = yScaleMode === "global" ? buffer.snapshot(channelId) : points;
    const channelRange = yScaleMode === "fixed" ? fixedRanges[channelId] ?? computeChannelRange(points.map((point) => point.value.value)) : computeChannelRange(rangePoints.map((point) => point.value.value));
    if (channelRange === null) return;
    const range = channelRange.max - channelRange.min;
    const buckets = minMaxBuckets(points.map((point) => ({ x: ((point.timestampUs - firstUs) / Math.max(1, latestUs - firstUs)) * (size.width - 1), y: point.value.value })), size.width);
    const style = channelStyle(slot);
    context.strokeStyle = style.color;
    context.setLineDash([...style.dash]);
    context.lineWidth = 1.25;
    const top = slot * bandHeight + 6;
    const yFor = (value: number) => top + (1 - (value - channelRange.min) / range) * (bandHeight - 12);
    context.beginPath();
    buckets.forEach((bucket, index) => {
      const y = yFor(bucket.last);
      if (index === 0) context.moveTo(bucket.x, y);
      else context.lineTo(bucket.x, y);
    });
    context.stroke();
    for (const bucket of buckets) {
      const yMin = yFor(bucket.min);
      const yMax = yFor(bucket.max);
      if (Math.abs(yMin - yMax) < 0.25) continue;
      context.beginPath(); context.moveTo(bucket.x, yMin); context.lineTo(bucket.x, yMax); context.stroke();
    }
    context.setLineDash([]);
    const descriptor = descriptors.find((item) => item.channelId === channelId);
    context.fillStyle = style.color;
    context.font = "10px ui-monospace";
    context.fillText(descriptor?.displayName ?? String(channelId), 5, slot * bandHeight + 11);
  });
  drawCursor(context, probeTimestampUs, "探针", "rgba(255,255,255,.55)", [2, 4], size, firstUs, latestUs);
  drawCursor(context, cursorAUs, "A", "#22d3ee", [], size, firstUs, latestUs);
  drawCursor(context, cursorBUs, "B", "#fbbf24", [8, 4], size, firstUs, latestUs);
}

function visibleBounds(buffer: TelemetryRingBuffer, selectedIds: number[], windowSeconds: number, viewportEndUs: number | null): { firstUs: number; latestUs: number } | null {
  const latestUs = viewportEndUs ?? Math.max(0, ...selectedIds.map((id) => buffer.latest(id)?.timestampUs ?? 0));
  if (latestUs === 0) return null;
  return { latestUs, firstUs: Math.max(0, latestUs - windowSeconds * 1_000_000) };
}

function drawCursor(context: CanvasRenderingContext2D, timestampUs: number | null, label: string, color: string, dash: number[], size: Size, firstUs: number, latestUs: number) {
  if (timestampUs === null || latestUs <= firstUs || timestampUs < firstUs || timestampUs > latestUs) return;
  const x = xForTimestamp(timestampUs, size.width, firstUs, latestUs);
  context.strokeStyle = color; context.fillStyle = color; context.setLineDash(dash); context.beginPath(); context.moveTo(x, 0); context.lineTo(x, size.height); context.stroke(); context.setLineDash([]); context.font = "10px ui-monospace"; context.fillText(label, Math.min(size.width - 16, x + 3), 11);
}
