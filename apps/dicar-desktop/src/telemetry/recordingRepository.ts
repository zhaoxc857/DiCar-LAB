import {
  MAX_RECORDING_COUNT,
  MAX_RECORDING_DURATION_MS,
  MAX_RECORDING_LIBRARY_BYTES,
  RECORDING_FORMAT,
  RECORDING_SCHEMA_VERSION,
  parseRecordingJson,
  rekeyImportedRecording,
  type RecordingStopReason,
  type TelemetryRecordingChunk,
  type TelemetryRecordingDocument,
  type TelemetryRecordingMetadata,
  type TelemetryRecordingStats,
} from "./recordings";

export const RECORDING_DATABASE_NAME = "dicar-tune-recordings";
export const RECORDING_DATABASE_VERSION = 1;

const RECORDINGS_STORE = "recordings";
const CHUNKS_STORE = "recordingChunks";
const RECORDING_ID_INDEX = "recordingId";

export type RecordingRepositoryOperation =
  | "create"
  | "append"
  | "seal"
  | "delete"
  | "importAfterMetadata";

export type RecordingRepositoryOptions = {
  indexedDb?: IDBFactory;
  databaseName?: string;
  maxCount?: number;
  maxBytes?: number;
  faultInjector?: (operation: RecordingRepositoryOperation) => void;
};

export class RecordingRepository {
  private readonly indexedDb: IDBFactory;
  private readonly databaseName: string;
  private readonly maxCount: number;
  private readonly maxBytes: number;
  private readonly faultInjector: ((operation: RecordingRepositoryOperation) => void) | undefined;
  private readonly protectionCounts = new Map<string, number>();
  private databasePromise: Promise<IDBDatabase> | null = null;
  private readyPromise: Promise<void> | null = null;
  private writeTail: Promise<void> = Promise.resolve();

  constructor(options: RecordingRepositoryOptions = {}) {
    this.indexedDb = options.indexedDb ?? indexedDB;
    this.databaseName = options.databaseName ?? RECORDING_DATABASE_NAME;
    this.maxCount = options.maxCount ?? MAX_RECORDING_COUNT;
    this.maxBytes = options.maxBytes ?? MAX_RECORDING_LIBRARY_BYTES;
    this.faultInjector = options.faultInjector;
    if (!Number.isInteger(this.maxCount) || this.maxCount < 1) throw new Error("记录数量上限无效");
    if (!Number.isSafeInteger(this.maxBytes) || this.maxBytes < 1) throw new Error("记录容量上限无效");
  }

  open(): Promise<void> {
    this.readyPromise ??= this.getDatabase().then((database) => this.enqueueWrite(
      () => this.cleanupIncompleteInternal(database),
    ));
    return this.readyPromise;
  }

  close(): void {
    void this.databasePromise?.then((database) => database.close());
  }

  protect(recordingId: string): () => void {
    this.protectionCounts.set(recordingId, (this.protectionCounts.get(recordingId) ?? 0) + 1);
    let released = false;
    return () => {
      if (released) return;
      released = true;
      const count = this.protectionCounts.get(recordingId) ?? 0;
      if (count <= 1) this.protectionCounts.delete(recordingId);
      else this.protectionCounts.set(recordingId, count - 1);
    };
  }

  async createRecording(metadata: TelemetryRecordingMetadata): Promise<void> {
    await this.open();
    if (metadata.status !== "recording" || metadata.stats.chunkCount !== 0) {
      throw new Error("只能创建空的录制中记录");
    }
    const database = await this.getDatabase();
    await this.enqueueWrite(async () => {
      this.faultInjector?.("create");
      const current = await this.getAllMetadata(database);
      if (current.some(({ id }) => id === metadata.id)) throw new Error("记录 ID 已存在");
      const victims = this.planPrune([...current, metadata], metadata.id);
      await this.runWriteTransaction(database, async (transaction) => {
        await this.deleteVictims(transaction, victims);
        transaction.objectStore(RECORDINGS_STORE).add(structuredClone(metadata));
      });
    });
  }

  async appendChunk(chunk: TelemetryRecordingChunk): Promise<TelemetryRecordingMetadata> {
    await this.open();
    const database = await this.getDatabase();
    return this.enqueueWrite(async () => {
      try {
        this.faultInjector?.("append");
        const metadata = await this.getMetadataInternal(database, chunk.recordingId);
        if (metadata?.status !== "recording") throw new Error("录制记录不存在或已封存");
        const updated = {
          ...metadata,
          stats: addChunkStats(metadata.stats, chunk),
        };
        const current = await this.getAllMetadata(database);
        const projected = current.map((item) => item.id === updated.id ? updated : item);
        const victims = this.planPrune(projected, updated.id);
        await this.runWriteTransaction(database, async (transaction) => {
          await this.deleteVictims(transaction, victims);
          transaction.objectStore(CHUNKS_STORE).add(structuredClone(chunk));
          transaction.objectStore(RECORDINGS_STORE).put(structuredClone(updated));
        });
        return updated;
      } catch (error) {
        try {
          await this.deleteRecordingInternal(database, chunk.recordingId, false);
        } catch (cleanupError) {
          throw new Error(
            `记录写入失败，且无法清理未完成记录：${errorMessage(cleanupError)}`,
            { cause: error },
          );
        }
        throw error;
      }
    });
  }

  async sealRecording(
    recordingId: string,
    stopReason: RecordingStopReason,
    completedAtMs: number,
    markers: readonly string[],
  ): Promise<TelemetryRecordingMetadata> {
    await this.open();
    const database = await this.getDatabase();
    return this.enqueueWrite(async () => {
      this.faultInjector?.("seal");
      const metadata = await this.getMetadataInternal(database, recordingId);
      if (metadata?.status !== "recording") throw new Error("录制记录不存在或已封存");
      const durationMs = completedAtMs - metadata.createdAtMs;
      if (!Number.isFinite(completedAtMs) || durationMs < 0 || durationMs > MAX_RECORDING_DURATION_MS) {
        throw new Error("录制完成时间无效");
      }
      const complete: TelemetryRecordingMetadata = {
        ...metadata,
        status: "complete",
        completedAtMs,
        stopReason,
        markers: [...markers],
      };
      await this.runWriteTransaction(database, (transaction) => {
        transaction.objectStore(RECORDINGS_STORE).put(structuredClone(complete));
      });
      return complete;
    });
  }

  async deleteRecording(recordingId: string): Promise<void> {
    await this.open();
    if (this.isProtected(recordingId)) throw new Error("正在回放或导出的记录不能删除");
    const database = await this.getDatabase();
    await this.enqueueWrite(() => this.deleteRecordingInternal(database, recordingId, true));
  }

  async getMetadata(recordingId: string): Promise<TelemetryRecordingMetadata | null> {
    await this.open();
    return this.getMetadataInternal(await this.getDatabase(), recordingId);
  }

  async listRecordings(): Promise<TelemetryRecordingMetadata[]> {
    await this.open();
    return (await this.getAllMetadata(await this.getDatabase()))
      .filter(({ status }) => status === "complete")
      .sort(compareNewestFirst);
  }

  async getChunks(recordingId: string): Promise<TelemetryRecordingChunk[]> {
    await this.open();
    return this.getChunksInternal(await this.getDatabase(), recordingId);
  }

  async getDocument(recordingId: string): Promise<TelemetryRecordingDocument | null> {
    await this.open();
    const database = await this.getDatabase();
    const transaction = database.transaction([RECORDINGS_STORE, CHUNKS_STORE], "readonly");
    const completion = transactionDone(transaction);
    const metadata = await requestToPromise<TelemetryRecordingMetadata | undefined>(
      transaction.objectStore(RECORDINGS_STORE).get(recordingId),
    );
    const chunks = await requestToPromise<TelemetryRecordingChunk[]>(
      transaction.objectStore(CHUNKS_STORE).index(RECORDING_ID_INDEX).getAll(recordingId),
    );
    await completion;
    if (metadata?.status !== "complete") return null;
    chunks.sort((left, right) => left.chunkIndex - right.chunkIndex);
    return {
      format: RECORDING_FORMAT,
      schemaVersion: RECORDING_SCHEMA_VERSION,
      metadata: structuredClone(metadata),
      chunks: structuredClone(chunks),
    };
  }

  async importJson(
    text: string,
    fileSize?: number,
    idFactory: () => string = () => crypto.randomUUID(),
  ): Promise<TelemetryRecordingDocument> {
    const parsed = parseRecordingJson(text, fileSize);
    await this.open();
    const database = await this.getDatabase();
    return this.enqueueWrite(async () => {
      const current = await this.getAllMetadata(database);
      const document = rekeyImportedRecording(parsed, new Set(current.map(({ id }) => id)), idFactory);
      const victims = this.planPrune([...current, document.metadata], document.metadata.id);
      await this.runWriteTransaction(database, async (transaction) => {
        await this.deleteVictims(transaction, victims);
        transaction.objectStore(RECORDINGS_STORE).add(structuredClone(document.metadata));
        this.faultInjector?.("importAfterMetadata");
        const chunks = transaction.objectStore(CHUNKS_STORE);
        for (const chunk of document.chunks) chunks.add(structuredClone(chunk));
      });
      return structuredClone(document);
    });
  }

  async prune(): Promise<void> {
    await this.open();
    const database = await this.getDatabase();
    await this.enqueueWrite(async () => {
      const current = await this.getAllMetadata(database);
      const victims = this.planPrune(current);
      if (victims.length === 0) return;
      await this.runWriteTransaction(database, (transaction) => this.deleteVictims(transaction, victims));
    });
  }

  private getDatabase(): Promise<IDBDatabase> {
    this.databasePromise ??= new Promise((resolve, reject) => {
      const request = this.indexedDb.open(this.databaseName, RECORDING_DATABASE_VERSION);
      request.onupgradeneeded = () => {
        const database = request.result;
        const recordings = database.createObjectStore(RECORDINGS_STORE, { keyPath: "id" });
        recordings.createIndex("createdAtMs", "createdAtMs");
        const chunks = database.createObjectStore(CHUNKS_STORE, {
          keyPath: ["recordingId", "chunkIndex"],
        });
        chunks.createIndex(RECORDING_ID_INDEX, "recordingId");
      };
      request.onerror = () => reject(request.error ?? new Error("无法打开记录数据库"));
      request.onblocked = () => reject(new Error("记录数据库升级被阻塞"));
      request.onsuccess = () => resolve(request.result);
    });
    return this.databasePromise;
  }

  private enqueueWrite<T>(work: () => Promise<T>): Promise<T> {
    const result = this.writeTail.then(work, work);
    this.writeTail = result.then(() => undefined, () => undefined);
    return result;
  }

  private async cleanupIncompleteInternal(database: IDBDatabase): Promise<void> {
    const current = await this.getAllMetadata(database);
    const incomplete = current.filter(({ status }) => status !== "complete").map(({ id }) => id);
    if (incomplete.length === 0) return;
    await this.runWriteTransaction(database, (transaction) => this.deleteVictims(transaction, incomplete));
  }

  private async deleteRecordingInternal(database: IDBDatabase, recordingId: string, injectFault: boolean): Promise<void> {
    if (injectFault) this.faultInjector?.("delete");
    await this.runWriteTransaction(database, (transaction) => this.deleteVictims(transaction, [recordingId]));
  }

  private async deleteVictims(transaction: IDBTransaction, recordingIds: readonly string[]): Promise<void> {
    const recordings = transaction.objectStore(RECORDINGS_STORE);
    const chunks = transaction.objectStore(CHUNKS_STORE);
    for (const recordingId of recordingIds) {
      recordings.delete(recordingId);
      await deleteChunks(chunks, recordingId);
    }
  }

  private async runWriteTransaction(
    database: IDBDatabase,
    work: (transaction: IDBTransaction) => void | Promise<void>,
  ): Promise<void> {
    const transaction = database.transaction([RECORDINGS_STORE, CHUNKS_STORE], "readwrite");
    const completion = transactionDone(transaction);
    try {
      await work(transaction);
      await completion;
    } catch (error) {
      try {
        transaction.abort();
      } catch {
        // The transaction may already have aborted after a failed request.
      }
      try {
        await completion;
      } catch {
        // Preserve the original failure.
      }
      throw error;
    }
  }

  private async getMetadataInternal(database: IDBDatabase, recordingId: string): Promise<TelemetryRecordingMetadata | null> {
    const transaction = database.transaction(RECORDINGS_STORE, "readonly");
    const completion = transactionDone(transaction);
    const result = await requestToPromise<TelemetryRecordingMetadata | undefined>(
      transaction.objectStore(RECORDINGS_STORE).get(recordingId),
    );
    await completion;
    return result === undefined ? null : structuredClone(result);
  }

  private async getAllMetadata(database: IDBDatabase): Promise<TelemetryRecordingMetadata[]> {
    const transaction = database.transaction(RECORDINGS_STORE, "readonly");
    const completion = transactionDone(transaction);
    const result = await requestToPromise<TelemetryRecordingMetadata[]>(
      transaction.objectStore(RECORDINGS_STORE).getAll(),
    );
    await completion;
    return structuredClone(result);
  }

  private async getChunksInternal(database: IDBDatabase, recordingId: string): Promise<TelemetryRecordingChunk[]> {
    const transaction = database.transaction(CHUNKS_STORE, "readonly");
    const completion = transactionDone(transaction);
    const result = await requestToPromise<TelemetryRecordingChunk[]>(
      transaction.objectStore(CHUNKS_STORE).index(RECORDING_ID_INDEX).getAll(recordingId),
    );
    await completion;
    return structuredClone(result).sort((left, right) => left.chunkIndex - right.chunkIndex);
  }

  private planPrune(metadata: readonly TelemetryRecordingMetadata[], protectedExtraId?: string): string[] {
    let count = metadata.length;
    let bytes = metadata.reduce((total, item) => total + item.stats.logicalBytes, 0);
    const candidates = metadata
      .filter((item) => item.status === "complete"
        && item.id !== protectedExtraId
        && !this.isProtected(item.id))
      .sort(compareOldestFirst);
    const victims: string[] = [];
    for (const candidate of candidates) {
      if (count <= this.maxCount && bytes <= this.maxBytes) break;
      victims.push(candidate.id);
      count -= 1;
      bytes -= candidate.stats.logicalBytes;
    }
    if (count > this.maxCount || bytes > this.maxBytes) {
      throw new Error("记录库容量不足，且没有可自动清理的完整记录");
    }
    return victims;
  }

  private isProtected(recordingId: string): boolean {
    return (this.protectionCounts.get(recordingId) ?? 0) > 0;
  }
}

function addChunkStats(stats: TelemetryRecordingStats, chunk: TelemetryRecordingChunk): TelemetryRecordingStats {
  let batchCount = 0;
  let pointCount = 0;
  let droppedSamples = 0;
  let firstTimestampUs: number | null = null;
  let lastTimestampUs: number | null = null;
  for (const batch of chunk.batches) {
    batchCount += 1;
    pointCount += batch.points.length;
    droppedSamples += batch.droppedSamples;
    for (const point of batch.points) {
      firstTimestampUs = firstTimestampUs === null ? point.timestampUs : Math.min(firstTimestampUs, point.timestampUs);
      lastTimestampUs = lastTimestampUs === null ? point.timestampUs : Math.max(lastTimestampUs, point.timestampUs);
    }
  }
  return {
    batchCount: stats.batchCount + batchCount,
    pointCount: stats.pointCount + pointCount,
    droppedSamples: stats.droppedSamples + droppedSamples,
    firstTimestampUs: minimumTimestamp(stats.firstTimestampUs, firstTimestampUs),
    lastTimestampUs: maximumTimestamp(stats.lastTimestampUs, lastTimestampUs),
    chunkCount: stats.chunkCount + 1,
    logicalBytes: stats.logicalBytes + chunk.logicalBytes,
  };
}

function minimumTimestamp(left: number | null, right: number | null): number | null {
  if (left === null) return right;
  if (right === null) return left;
  return Math.min(left, right);
}

function maximumTimestamp(left: number | null, right: number | null): number | null {
  if (left === null) return right;
  if (right === null) return left;
  return Math.max(left, right);
}

function compareNewestFirst(left: TelemetryRecordingMetadata, right: TelemetryRecordingMetadata): number {
  return right.createdAtMs - left.createdAtMs || right.id.localeCompare(left.id);
}

function compareOldestFirst(left: TelemetryRecordingMetadata, right: TelemetryRecordingMetadata): number {
  return left.createdAtMs - right.createdAtMs || left.id.localeCompare(right.id);
}

function requestToPromise<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("记录数据库请求失败"));
  });
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("记录数据库事务失败"));
    transaction.onabort = () => reject(transaction.error ?? new Error("记录数据库事务已取消"));
  });
}

async function deleteChunks(store: IDBObjectStore, recordingId: string): Promise<void> {
  const keys = await requestToPromise<IDBValidKey[]>(
    store.index(RECORDING_ID_INDEX).getAllKeys(recordingId),
  );
  for (const key of keys) store.delete(key);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "未知错误";
}
