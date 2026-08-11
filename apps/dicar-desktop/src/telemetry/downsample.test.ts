import { minMaxBuckets } from "./downsample";

it("preserves extrema when reducing many samples to one pixel column", () => {
  const buckets = minMaxBuckets([{ x: 0, y: -4 }, { x: 0, y: 9 }, { x: 1, y: 2 }], 2);
  expect(buckets[0]).toMatchObject({ x: 0, min: -4, max: 9 });
  expect(buckets[1]).toMatchObject({ x: 1, min: 2, max: 2 });
});

it.each([30_000, 240_000])("bounds %i input points to two draw vertices per CSS pixel", (count) => {
  const width = 640;
  const points = Array.from({ length: count }, (_, index) => ({ x: (index / Math.max(1, count - 1)) * (width - 1), y: Math.sin(index / 17) * 10 }));
  const buckets = minMaxBuckets(points, width);
  expect(buckets.length).toBeLessThanOrEqual(width);
  expect(buckets.length * 2).toBeLessThanOrEqual(width * 2);
  expect(Math.max(...buckets.map(({ max }) => max))).toBeGreaterThan(9.9);
  expect(Math.min(...buckets.map(({ min }) => min))).toBeLessThan(-9.9);
});

it("returns no buckets for invalid width or empty input", () => {
  expect(minMaxBuckets([], 100)).toEqual([]);
  expect(minMaxBuckets([{ x: 0, y: 1 }], 0)).toEqual([]);
});
