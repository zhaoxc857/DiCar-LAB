export type PlotPoint = { x: number; y: number };
export type MinMaxBucket = { x: number; min: number; max: number; first: number; last: number };

export function minMaxBuckets(points: readonly PlotPoint[], cssPixelWidth: number): MinMaxBucket[] {
  const width = Math.floor(cssPixelWidth);
  if (width <= 0 || points.length === 0) return [];
  const slots: Array<MinMaxBucket | undefined> = new Array(width);
  for (const point of points) {
    if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) continue;
    const x = Math.max(0, Math.min(width - 1, Math.floor(point.x)));
    const bucket = slots[x];
    if (bucket) {
      bucket.min = Math.min(bucket.min, point.y);
      bucket.max = Math.max(bucket.max, point.y);
      bucket.last = point.y;
    } else slots[x] = { x, min: point.y, max: point.y, first: point.y, last: point.y };
  }
  return slots.filter((bucket): bucket is MinMaxBucket => bucket !== undefined);
}
