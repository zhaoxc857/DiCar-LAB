import type { TelemetryPoint } from "../domain/types";
import { TelemetryRingBuffer } from "./ringBuffer";

function points(count: number, channelId = 200): TelemetryPoint[] {
  return Array.from({ length: count }, (_, index) => ({ channelId, timestampUs: index * 2_000, sampleSequence: index & 0xffff, value: { kind: "u32", value: index >>> 0 } }));
}

it("retains only sixty seconds per channel at five hundred hertz", () => {
  const buffer = new TelemetryRingBuffer(8, 30_000);
  const samples = points(31_000);
  buffer.append(samples);
  expect(buffer.length(200)).toBe(30_000);
  expect(buffer.first(200)?.timestampUs).toBe(samples[1_000].timestampUs);
  expect(buffer.latest(200)?.value).toEqual({ kind: "u32", value: 30_999 });
  expect(buffer.totalPoints).toBe(30_000);
});

it("keeps exact u32, flags, i32, and f32 values in preallocated channel storage", () => {
  const buffer = new TelemetryRingBuffer(8, 4);
  buffer.append([
    { channelId: 200, timestampUs: 1, sampleSequence: 1, value: { kind: "u32", value: 0xffff_ffff } },
    { channelId: 201, timestampUs: 1, sampleSequence: 1, value: { kind: "flags32", value: 0x8000_0001 } },
    { channelId: 202, timestampUs: 1, sampleSequence: 1, value: { kind: "i32", value: -2_147_483_648 } },
    { channelId: 203, timestampUs: 1, sampleSequence: 1, value: { kind: "f32", value: 1.25 } },
  ]);
  expect(buffer.latest(200)?.value).toEqual({ kind: "u32", value: 0xffff_ffff });
  expect(buffer.latest(201)?.value).toEqual({ kind: "flags32", value: 0x8000_0001 });
  expect(buffer.latest(202)?.value).toEqual({ kind: "i32", value: -2_147_483_648 });
  expect(buffer.latest(203)?.value).toEqual({ kind: "f32", value: 1.25 });
});

it("bounds channels and clears deterministically", () => {
  const buffer = new TelemetryRingBuffer(2, 3);
  buffer.append([...points(2, 200), ...points(2, 201)]);
  expect(() => buffer.append(points(1, 202))).toThrow("最多缓存 2 个遥测通道");
  buffer.clear();
  expect(buffer.totalPoints).toBe(0);
  expect(buffer.channelIds()).toEqual([]);
});
