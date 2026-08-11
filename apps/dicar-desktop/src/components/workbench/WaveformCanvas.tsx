import { useEffect, useRef, useState } from "react";
import type { TelemetryDescriptor } from "../../domain/types";
import { channelStyle } from "../../telemetry/channelStyles";
import { minMaxBuckets } from "../../telemetry/downsample";
import type { TelemetryRingBuffer } from "../../telemetry/ringBuffer";

type Size = { width: number; height: number };

export function WaveformCanvas({ buffer, descriptors, selectedIds, windowSeconds, cursorIndex, visualRevision, paused }: { buffer: TelemetryRingBuffer; descriptors: TelemetryDescriptor[]; selectedIds: number[]; windowSeconds: number; cursorIndex: number; visualRevision: number; paused: boolean }) {
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
    const frame = requestAnimationFrame(() => draw(canvas, size, buffer, descriptors, selectedIds, windowSeconds, cursorIndex));
    return () => cancelAnimationFrame(frame);
  }, [buffer, cursorIndex, descriptors, paused, selectedIds, size, visualRevision, windowSeconds]);

  return <canvas aria-label="实时波形" className="block h-48 w-full 2xl:h-64" ref={canvasRef} />;
}

function draw(canvas: HTMLCanvasElement, size: Size, buffer: TelemetryRingBuffer, descriptors: TelemetryDescriptor[], selectedIds: number[], windowSeconds: number, cursorIndex: number) {
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
  const latestUs = Math.max(0, ...selectedIds.map((id) => buffer.latest(id)?.timestampUs ?? 0));
  const firstUs = Math.max(0, latestUs - windowSeconds * 1_000_000);
  const bandHeight = Math.max(24, (size.height - 18) / Math.max(1, selectedIds.length));
  selectedIds.forEach((channelId, slot) => {
    const points = buffer.snapshot(channelId, firstUs);
    if (points.length === 0) return;
    const values = points.map((point) => point.value.value);
    const min = Math.min(...values);
    const max = Math.max(...values);
    const range = max === min ? 1 : max - min;
    const buckets = minMaxBuckets(points.map((point) => ({ x: ((point.timestampUs - firstUs) / Math.max(1, latestUs - firstUs)) * (size.width - 1), y: point.value.value })), size.width);
    const style = channelStyle(slot);
    context.strokeStyle = style.color;
    context.setLineDash([...style.dash]);
    context.lineWidth = 1.25;
    const top = slot * bandHeight + 6;
    const yFor = (value: number) => top + (1 - (value - min) / range) * (bandHeight - 12);
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
  const firstChannel = selectedIds[0];
  const cursor = firstChannel === undefined ? undefined : buffer.at(firstChannel, Math.max(0, Math.min(cursorIndex, buffer.length(firstChannel) - 1)));
  if (cursor && latestUs > firstUs) {
    const x = ((cursor.timestampUs - firstUs) / (latestUs - firstUs)) * size.width;
    context.strokeStyle = "rgba(255,255,255,.65)"; context.setLineDash([3, 3]); context.beginPath(); context.moveTo(x, 0); context.lineTo(x, size.height); context.stroke(); context.setLineDash([]);
  }
}
