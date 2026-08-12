import type { TelemetryPoint } from "../domain/types";
import type { TelemetryRingBuffer } from "./ringBuffer";

export type ActiveCursor = "A" | "B";
export type WaveformCursorState = {
  cursorAUs: number | null;
  cursorBUs: number | null;
  activeCursor: ActiveCursor;
};
export type ChannelRange = { min: number; max: number };
export type YScaleMode = "local" | "global" | "fixed";
export type NearestReading = { point: TelemetryPoint; offsetUs: number };

export function timestampForX(x: number, width: number, firstUs: number, latestUs: number): number {
  if (width <= 0 || latestUs <= firstUs) return firstUs;
  const ratio = clamp(x / width, 0, 1);
  return firstUs + ratio * (latestUs - firstUs);
}

export function xForTimestamp(timestampUs: number, width: number, firstUs: number, latestUs: number): number {
  if (width <= 0 || latestUs <= firstUs) return 0;
  return clamp(((timestampUs - firstUs) / (latestUs - firstUs)) * width, 0, width);
}

export function clickCursor(state: WaveformCursorState, timestampUs: number): WaveformCursorState {
  if (state.cursorAUs === null) return { cursorAUs: timestampUs, cursorBUs: null, activeCursor: "A" };
  if (state.cursorBUs === null) return { ...state, cursorBUs: timestampUs, activeCursor: "B" };
  return { cursorAUs: timestampUs, cursorBUs: null, activeCursor: "A" };
}

export function clampCursorsToBounds(
  state: WaveformCursorState,
  firstUs: number,
  latestUs: number,
): { state: WaveformCursorState; clamped: ActiveCursor[] } {
  if (latestUs < firstUs) return { state, clamped: [] };
  const clamped: ActiveCursor[] = [];
  const cursorAUs = clampCursor("A", state.cursorAUs);
  const cursorBUs = clampCursor("B", state.cursorBUs);
  if (clamped.length === 0) return { state, clamped };
  return { state: { ...state, cursorAUs, cursorBUs }, clamped };

  function clampCursor(cursor: ActiveCursor, timestampUs: number | null): number | null {
    if (timestampUs === null || (timestampUs >= firstUs && timestampUs <= latestUs)) return timestampUs;
    clamped.push(cursor);
    return clamp(timestampUs, firstUs, latestUs);
  }
}

export function advanceCursor(
  buffer: TelemetryRingBuffer,
  channelId: number,
  state: WaveformCursorState,
  direction: -1 | 1,
  step: number,
): WaveformCursorState {
  const length = buffer.length(channelId);
  if (length === 0) return state;
  const activeTimestamp = state.activeCursor === "B" ? state.cursorBUs : state.cursorAUs;
  if (activeTimestamp === null) {
    const latest = buffer.latest(channelId);
    return latest === undefined ? state : { ...state, cursorAUs: latest.timestampUs, activeCursor: "A" };
  }
  const currentIndex = buffer.indexAtOrNearest(channelId, activeTimestamp) ?? length - 1;
  const nextIndex = clamp(Math.round(currentIndex + direction * Math.max(1, step)), 0, length - 1);
  const timestampUs = buffer.at(channelId, nextIndex)?.timestampUs;
  if (timestampUs === undefined) return state;
  if (state.activeCursor === "B" && state.cursorBUs !== null) return { ...state, cursorBUs: timestampUs };
  return { ...state, cursorAUs: timestampUs, activeCursor: "A" };
}

export function nearestReading(buffer: TelemetryRingBuffer, channelId: number, timestampUs: number): NearestReading | null {
  const point = buffer.nearest(channelId, timestampUs);
  if (point === undefined) return null;
  const length = buffer.length(channelId);
  if (length === 1) return point.timestampUs === timestampUs ? { point, offsetUs: 0 } : null;
  const index = buffer.indexAtOrNearest(channelId, timestampUs) ?? 0;
  const before = buffer.at(channelId, Math.max(0, index - 1));
  const after = buffer.at(channelId, Math.min(length - 1, index + 1));
  const intervals = [before, after]
    .filter((candidate): candidate is TelemetryPoint => candidate !== undefined && candidate.timestampUs !== point.timestampUs)
    .map((candidate) => Math.abs(candidate.timestampUs - point.timestampUs));
  const observedInterval = intervals.length === 0 ? 0 : Math.min(...intervals);
  const offsetUs = point.timestampUs - timestampUs;
  return observedInterval > 0 && Math.abs(offsetUs) <= observedInterval * 2 ? { point, offsetUs } : null;
}

export function computeChannelRange(values: readonly number[]): ChannelRange | null {
  const finite = values.filter(Number.isFinite);
  if (finite.length === 0) return null;
  const min = Math.min(...finite);
  const max = Math.max(...finite);
  if (min === max) {
    const padding = Math.max(1, Math.abs(min) * 0.1);
    return { min: min - padding, max: max + padding };
  }
  const padding = (max - min) * 0.12;
  return { min: min - padding, max: max + padding };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
