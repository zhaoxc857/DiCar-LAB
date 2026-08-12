import type { TelemetryPoint, TelemetryValue } from "../domain/types";

type TelemetryKind = TelemetryValue["kind"];
type ValueStorage = Float32Array | Int32Array | Uint32Array;

class ChannelBuffer {
  readonly timestamps: Float64Array;
  readonly sequences: Uint16Array;
  readonly values: ValueStorage;
  start = 0;
  count = 0;

  constructor(readonly channelId: number, readonly kind: TelemetryKind, readonly capacity: number) {
    this.timestamps = new Float64Array(capacity);
    this.sequences = new Uint16Array(capacity);
    this.values = kind === "f32" ? new Float32Array(capacity) : kind === "i32" ? new Int32Array(capacity) : new Uint32Array(capacity);
  }

  append(point: TelemetryPoint): void {
    if (point.value.kind !== this.kind) throw new Error(`遥测通道 ${this.channelId} 类型从 ${this.kind} 变为 ${point.value.kind}`);
    const index = this.count < this.capacity ? (this.start + this.count) % this.capacity : this.start;
    this.timestamps[index] = point.timestampUs;
    this.sequences[index] = point.sampleSequence;
    this.values[index] = point.value.value;
    if (this.count < this.capacity) this.count += 1;
    else this.start = (this.start + 1) % this.capacity;
  }

  pointAt(index: number): TelemetryPoint | undefined {
    if (index < 0 || index >= this.count) return undefined;
    const physical = (this.start + index) % this.capacity;
    return { channelId: this.channelId, timestampUs: this.timestamps[physical], sampleSequence: this.sequences[physical], value: valueOf(this.kind, this.values[physical]) };
  }

  indexAtOrNearest(timestampUs: number): number | undefined {
    if (this.count === 0) return undefined;
    let low = 0;
    let high = this.count - 1;
    while (low <= high) {
      const middle = Math.floor((low + high) / 2);
      const timestamp = this.pointAt(middle)?.timestampUs ?? timestampUs;
      if (timestamp < timestampUs) low = middle + 1;
      else if (timestamp > timestampUs) high = middle - 1;
      else return middle;
    }
    if (low <= 0) return 0;
    if (low >= this.count) return this.count - 1;
    const before = this.pointAt(low - 1)?.timestampUs ?? timestampUs;
    const after = this.pointAt(low)?.timestampUs ?? timestampUs;
    return timestampUs - before <= after - timestampUs ? low - 1 : low;
  }
}

export class TelemetryRingBuffer {
  readonly #channels = new Map<number, ChannelBuffer>();

  constructor(readonly maxChannels: number, readonly capacityPerChannel: number) {
    if (!Number.isInteger(maxChannels) || maxChannels < 1) throw new Error("maxChannels 必须为正整数");
    if (!Number.isInteger(capacityPerChannel) || capacityPerChannel < 1) throw new Error("capacityPerChannel 必须为正整数");
  }

  append(points: readonly TelemetryPoint[]): void {
    const newIds = new Set<number>();
    for (const point of points) if (!this.#channels.has(point.channelId)) newIds.add(point.channelId);
    if (this.#channels.size + newIds.size > this.maxChannels) throw new Error(`最多缓存 ${this.maxChannels} 个遥测通道`);
    for (const point of points) {
      let channel = this.#channels.get(point.channelId);
      if (!channel) {
        channel = new ChannelBuffer(point.channelId, point.value.kind, this.capacityPerChannel);
        this.#channels.set(point.channelId, channel);
      }
      channel.append(point);
    }
  }

  get totalPoints(): number {
    let total = 0;
    for (const channel of this.#channels.values()) total += channel.count;
    return total;
  }

  channelIds(): number[] { return [...this.#channels.keys()]; }
  length(channelId: number): number { return this.#channels.get(channelId)?.count ?? 0; }
  first(channelId: number): TelemetryPoint | undefined { return this.#channels.get(channelId)?.pointAt(0); }
  latest(channelId: number): TelemetryPoint | undefined { const channel = this.#channels.get(channelId); return channel?.pointAt(channel.count - 1); }
  at(channelId: number, index: number): TelemetryPoint | undefined { return this.#channels.get(channelId)?.pointAt(index); }
  indexAtOrNearest(channelId: number, timestampUs: number): number | undefined { return this.#channels.get(channelId)?.indexAtOrNearest(timestampUs); }
  nearest(channelId: number, timestampUs: number): TelemetryPoint | undefined {
    const channel = this.#channels.get(channelId);
    const index = channel?.indexAtOrNearest(timestampUs);
    return index === undefined ? undefined : channel?.pointAt(index);
  }

  snapshot(channelId: number, fromTimestampUs = Number.NEGATIVE_INFINITY): TelemetryPoint[] {
    const channel = this.#channels.get(channelId);
    if (!channel) return [];
    const result: TelemetryPoint[] = [];
    for (let index = 0; index < channel.count; index += 1) {
      const point = channel.pointAt(index);
      if (point && point.timestampUs >= fromTimestampUs) result.push(point);
    }
    return result;
  }

  clear(): void { this.#channels.clear(); }
}

function valueOf(kind: TelemetryKind, value: number): TelemetryValue {
  switch (kind) {
    case "f32": return { kind, value };
    case "i32": return { kind, value };
    case "u32": return { kind, value };
    case "flags32": return { kind, value };
  }
}
