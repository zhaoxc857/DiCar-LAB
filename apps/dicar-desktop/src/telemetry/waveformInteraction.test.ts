import type { TelemetryPoint } from "../domain/types";
import { TelemetryRingBuffer } from "./ringBuffer";
import {
  advanceCursor,
  clickCursor,
  computeChannelRange,
  nearestReading,
  timestampForX,
  xForTimestamp,
  type WaveformCursorState,
} from "./waveformInteraction";

const empty: WaveformCursorState = { cursorAUs: null, cursorBUs: null, activeCursor: "A" };

it("maps pointer coordinates to bounded timestamps and back", () => {
  expect(timestampForX(-20, 200, 1_000, 2_000)).toBe(1_000);
  expect(timestampForX(50, 200, 1_000, 2_000)).toBe(1_250);
  expect(timestampForX(300, 200, 1_000, 2_000)).toBe(2_000);
  expect(xForTimestamp(1_750, 200, 1_000, 2_000)).toBe(150);
  expect(xForTimestamp(900, 200, 1_000, 2_000)).toBe(0);
});

it("cycles A, B, then a replacement A using timestamp truth", () => {
  const a = clickCursor(empty, 10);
  const ab = clickCursor(a, 30);
  expect(a).toEqual({ cursorAUs: 10, cursorBUs: null, activeCursor: "A" });
  expect(ab).toEqual({ cursorAUs: 10, cursorBUs: 30, activeCursor: "B" });
  expect(clickCursor(ab, 50)).toEqual({ cursorAUs: 50, cursorBUs: null, activeCursor: "A" });
});

it("moves the active cursor by one or ten real samples and clamps at the buffer edge", () => {
  const buffer = sampleBuffer(Array.from({ length: 15 }, (_, index) => index * 100));
  const a = { cursorAUs: 500, cursorBUs: null, activeCursor: "A" } as const;
  expect(advanceCursor(buffer, 7, a, -1, 1).cursorAUs).toBe(400);
  expect(advanceCursor(buffer, 7, a, 1, 10).cursorAUs).toBe(1_400);
  expect(advanceCursor(buffer, 7, a, -1, 10).cursorAUs).toBe(0);
  expect(advanceCursor(buffer, 7, empty, -1, 1).cursorAUs).toBe(1_400);
});

it("returns an independent nearest sample only within twice the observed interval", () => {
  const buffer = sampleBuffer([0, 100, 200]);
  expect(nearestReading(buffer, 7, 149)).toMatchObject({ point: { timestampUs: 100 }, offsetUs: -49 });
  expect(nearestReading(buffer, 7, 450)).toBeNull();
  const single = sampleBuffer([100]);
  expect(nearestReading(single, 7, 100)?.point.timestampUs).toBe(100);
  expect(nearestReading(single, 7, 101)).toBeNull();
});

it("computes padded finite ranges for constant and varying channel values", () => {
  expect(computeChannelRange([5, 5, Number.NaN])).toEqual({ min: 4, max: 6 });
  expect(computeChannelRange([-10, 30, Number.POSITIVE_INFINITY])).toEqual({ min: -14.8, max: 34.8 });
  expect(computeChannelRange([Number.NaN])).toBeNull();
});

function sampleBuffer(timestamps: number[]): TelemetryRingBuffer {
  const buffer = new TelemetryRingBuffer(1, 30);
  buffer.append(timestamps.map((timestampUs, index): TelemetryPoint => ({
    channelId: 7,
    timestampUs,
    sampleSequence: index,
    value: { kind: "f32", value: index },
  })));
  return buffer;
}
