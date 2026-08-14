import type { AppSnapshot, BridgeEvent, UiTelemetryBatch } from "../domain/types";
import { RecordingRepository } from "../telemetry/recordingRepository";
import {
  MAX_RECORDING_DURATION_MS,
  RECORDING_CHUNK_POINT_LIMIT,
  RECORDING_CHUNK_SPAN_US,
  createRecordingChunk,
  createRecordingMetadata,
  recordingStartDenial,
  type RecordingStopReason,
  type TelemetryRecordingDocument,
  type TelemetryRecordingMetadata,
} from "../telemetry/recordings";

export type ActiveRecordingState = {
  id: string;
  name: string;
  startedAtMs: number;
  batchCount: number;
  pointCount: number;
};

export type RecordingControllerState = {
  status: "idle" | "recording" | "stopping" | "error";
  active: ActiveRecordingState | null;
  lastStopReason: RecordingStopReason | null;
  notice: string | null;
  error: string | null;
};

export type StartRecordingInput = {
  name: string;
  note: string;
  vehicleProfileId: string;
};

export type RecordingControllerOptions = {
  now?: () => number;
  idFactory?: () => string;
  scheduleTimeout?: (callback: () => void, delayMs: number) => ReturnType<typeof setTimeout>;
  cancelTimeout?: (handle: ReturnType<typeof setTimeout>) => void;
};

type ActiveRecording = {
  metadata: TelemetryRecordingMetadata;
  subscriptionVersion: number;
  baselineMarkers: string[];
  pendingBatches: UiTelemetryBatch[];
  pendingPointCount: number;
  pendingFirstTimestampUs: number | null;
  pendingLastTimestampUs: number | null;
  nextChunkIndex: number;
};

const INITIAL_STATE: RecordingControllerState = {
  status: "idle",
  active: null,
  lastStopReason: null,
  notice: null,
  error: null,
};

export class RecordingController {
  private readonly now: () => number;
  private readonly idFactory: () => string;
  private readonly scheduleTimeout: NonNullable<RecordingControllerOptions["scheduleTimeout"]>;
  private readonly cancelTimeout: NonNullable<RecordingControllerOptions["cancelTimeout"]>;
  private readonly listeners = new Set<() => void>();
  private state: RecordingControllerState = { ...INITIAL_STATE };
  private snapshot: AppSnapshot | null = null;
  private active: ActiveRecording | null = null;
  private durationTimer: ReturnType<typeof setTimeout> | null = null;
  private tail: Promise<void> = Promise.resolve();

  constructor(
    private readonly repository: RecordingRepository,
    options: RecordingControllerOptions = {},
  ) {
    this.now = options.now ?? Date.now;
    this.idFactory = options.idFactory ?? (() => crypto.randomUUID());
    this.scheduleTimeout = options.scheduleTimeout ?? ((callback, delayMs) => setTimeout(callback, delayMs));
    this.cancelTimeout = options.cancelTimeout ?? ((handle) => clearTimeout(handle));
  }

  getState = (): RecordingControllerState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  async initialize(): Promise<void> {
    try {
      await this.repository.open();
    } catch {
      this.setState({ status: "error", error: "无法打开波形记录库" });
      throw new Error("无法打开波形记录库");
    }
  }

  setSnapshot(snapshot: AppSnapshot): void {
    this.snapshot = structuredClone(snapshot);
  }

  start(input: StartRecordingInput): Promise<void> {
    return this.enqueue(async () => {
      if (this.active !== null) throw new Error("已有波形记录正在录制");
      const denial = recordingStartDenial(this.snapshot, input.name, input.note);
      if (denial !== null || this.snapshot === null) throw new Error(denial ?? "设备快照不可用");
      const createdAtMs = this.now();
      const metadata = createRecordingMetadata({
        id: this.idFactory(),
        name: input.name,
        note: input.note,
        snapshot: this.snapshot,
        vehicleProfileId: input.vehicleProfileId,
        createdAtMs,
      });
      try {
        await this.repository.createRecording(metadata);
      } catch {
        this.setState({ status: "error", active: null, error: "无法创建波形记录" });
        throw new Error("无法创建波形记录");
      }
      this.active = {
        metadata,
        subscriptionVersion: metadata.subscription.subscriptionVersion,
        baselineMarkers: [...this.snapshot.markers],
        pendingBatches: [],
        pendingPointCount: 0,
        pendingFirstTimestampUs: null,
        pendingLastTimestampUs: null,
        nextChunkIndex: 0,
      };
      this.durationTimer = this.scheduleTimeout(() => {
        void this.stop("durationLimit").catch(() => undefined);
      }, MAX_RECORDING_DURATION_MS);
      this.setState({
        status: "recording",
        active: toActiveState(this.active),
        lastStopReason: null,
        notice: null,
        error: null,
      });
    });
  }

  stop(reason: RecordingStopReason = "manual"): Promise<void> {
    return this.enqueue(() => this.stopInternal(reason));
  }

  acceptEvent(event: BridgeEvent): void {
    const immutableEvent = structuredClone(event);
    void this.enqueue(async () => {
      try {
        await this.acceptEventInternal(immutableEvent);
      } catch {
        await this.failActiveRecording();
      }
    });
  }

  async drain(): Promise<void> {
    await this.tail;
  }

  listRecordings(): Promise<TelemetryRecordingMetadata[]> {
    return this.repository.listRecordings();
  }

  getDocument(recordingId: string): Promise<TelemetryRecordingDocument | null> {
    return this.repository.getDocument(recordingId);
  }

  deleteRecording(recordingId: string): Promise<void> {
    return this.repository.deleteRecording(recordingId);
  }

  importJson(text: string, fileSize?: number): Promise<TelemetryRecordingDocument> {
    return this.repository.importJson(text, fileSize);
  }

  protect(recordingId: string): () => void {
    return this.repository.protect(recordingId);
  }

  private enqueue<T>(work: () => Promise<T>): Promise<T> {
    const result = this.tail.then(work, work);
    this.tail = result.then(() => undefined, () => undefined);
    return result;
  }

  private async acceptEventInternal(event: BridgeEvent): Promise<void> {
    if (event.event === "snapshotChanged") {
      this.snapshot = event.data;
      if (this.active === null) return;
      if (event.data.paused) {
        await this.stopInternal("paused");
      } else if (event.data.phase !== "ready") {
        await this.stopInternal("connectionLost");
      } else if (event.data.activeSubscription?.subscriptionVersion !== this.active.subscriptionVersion) {
        await this.stopInternal("subscriptionChanged");
      }
      return;
    }
    if (event.event === "connectionLost") {
      if (this.active !== null) await this.stopInternal("connectionLost");
      return;
    }
    if (event.event !== "telemetryBatch" || this.active === null) return;
    if (event.data.subscriptionVersion !== this.active.subscriptionVersion) {
      await this.stopInternal("subscriptionChanged");
      return;
    }
    this.appendPending(event.data);
    if (this.shouldFlush()) await this.flushPending();
    if (this.active !== null) this.setState({ active: toActiveState(this.active) });
  }

  private appendPending(batch: UiTelemetryBatch): void {
    const active = this.active;
    if (active === null) return;
    active.pendingBatches.push(batch);
    active.pendingPointCount += batch.points.length;
    for (const point of batch.points) {
      active.pendingFirstTimestampUs = active.pendingFirstTimestampUs === null
        ? point.timestampUs
        : Math.min(active.pendingFirstTimestampUs, point.timestampUs);
      active.pendingLastTimestampUs = active.pendingLastTimestampUs === null
        ? point.timestampUs
        : Math.max(active.pendingLastTimestampUs, point.timestampUs);
    }
  }

  private shouldFlush(): boolean {
    const active = this.active;
    if (active === null) return false;
    const spanUs = active.pendingFirstTimestampUs === null || active.pendingLastTimestampUs === null
      ? 0
      : active.pendingLastTimestampUs - active.pendingFirstTimestampUs;
    return active.pendingPointCount >= RECORDING_CHUNK_POINT_LIMIT || spanUs >= RECORDING_CHUNK_SPAN_US;
  }

  private async flushPending(): Promise<void> {
    const active = this.active;
    if (active === null || active.pendingBatches.length === 0) return;
    const chunk = createRecordingChunk(active.metadata.id, active.nextChunkIndex, active.pendingBatches);
    const updated = await this.repository.appendChunk(chunk);
    active.metadata = updated;
    active.nextChunkIndex += 1;
    active.pendingBatches = [];
    active.pendingPointCount = 0;
    active.pendingFirstTimestampUs = null;
    active.pendingLastTimestampUs = null;
  }

  private async stopInternal(reason: RecordingStopReason): Promise<void> {
    const active = this.active;
    if (active === null) return;
    this.clearDurationTimer();
    this.setState({ status: "stopping" });
    try {
      await this.flushPending();
      if (this.active === null) return;
      const completedAtMs = Math.min(this.now(), active.metadata.createdAtMs + MAX_RECORDING_DURATION_MS);
      const markers = addedMarkers(active.baselineMarkers, this.snapshot?.markers ?? []);
      await this.repository.sealRecording(active.metadata.id, reason, completedAtMs, markers);
      this.active = null;
      this.setState({
        status: "idle",
        active: null,
        lastStopReason: reason,
        notice: stopNotice(reason),
        error: null,
      });
    } catch (error) {
      await this.failActiveRecording();
      throw error;
    }
  }

  private async failActiveRecording(): Promise<void> {
    const recordingId = this.active?.metadata.id ?? null;
    this.active = null;
    this.clearDurationTimer();
    if (recordingId !== null) {
      try {
        await this.repository.deleteRecording(recordingId);
      } catch {
        this.setState({
          status: "error",
          active: null,
          error: "波形记录写入失败，且未能清理损坏记录",
        });
        return;
      }
    }
    this.setState({
      status: "error",
      active: null,
      error: "波形记录写入失败，已删除本次记录",
    });
  }

  private clearDurationTimer(): void {
    if (this.durationTimer === null) return;
    this.cancelTimeout(this.durationTimer);
    this.durationTimer = null;
  }

  private setState(patch: Partial<RecordingControllerState>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) listener();
  }
}

let defaultController: RecordingController | null = null;

export function getDefaultRecordingController(): RecordingController {
  defaultController ??= new RecordingController(new RecordingRepository());
  return defaultController;
}

function toActiveState(active: ActiveRecording): ActiveRecordingState {
  return {
    id: active.metadata.id,
    name: active.metadata.name,
    startedAtMs: active.metadata.createdAtMs,
    batchCount: active.metadata.stats.batchCount + active.pendingBatches.length,
    pointCount: active.metadata.stats.pointCount + active.pendingPointCount,
  };
}

function addedMarkers(baseline: readonly string[], current: readonly string[]): string[] {
  const prefixUnchanged = baseline.every((marker, index) => current[index] === marker);
  if (prefixUnchanged) return current.slice(baseline.length);
  const remaining = [...baseline];
  return current.filter((marker) => {
    const index = remaining.indexOf(marker);
    if (index < 0) return true;
    remaining.splice(index, 1);
    return false;
  });
}

function stopNotice(reason: RecordingStopReason): string {
  switch (reason) {
    case "manual": return "波形记录已保存";
    case "durationLimit": return "已达到 5 分钟上限，记录已自动保存";
    case "paused": return "波形暂停，记录已自动保存";
    case "connectionLost": return "连接中断，记录已自动保存";
    case "subscriptionChanged": return "遥测订阅变化，记录已自动保存";
  }
}
