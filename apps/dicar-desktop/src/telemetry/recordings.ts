import type {
  AppSnapshot,
  ParameterValue,
  TelemetryDescriptor,
  TelemetrySubscriptionSnapshot,
  TelemetryValue,
  TransportIdentity,
  UiTelemetryBatch,
} from "../domain/types";

export const RECORDING_SCHEMA_VERSION = 1 as const;
export const RECORDING_FORMAT = "dicar-telemetry-recording" as const;
export const MAX_RECORDING_DURATION_MS = 5 * 60 * 1000;
export const MAX_RECORDING_COUNT = 20;
export const MAX_RECORDING_LIBRARY_BYTES = 256 * 1024 * 1024;
export const RECORDING_CHUNK_POINT_LIMIT = 4096;
export const RECORDING_CHUNK_SPAN_US = 1_000_000;

export type RecordingStopReason =
  | "manual"
  | "durationLimit"
  | "paused"
  | "connectionLost"
  | "subscriptionChanged";

export type RecordingParameterSnapshot = {
  paramId: number;
  machineName: string;
  ramValue: ParameterValue;
  revision: number;
};

export type TelemetryRecordingStats = {
  batchCount: number;
  pointCount: number;
  droppedSamples: number;
  firstTimestampUs: number | null;
  lastTimestampUs: number | null;
  chunkCount: number;
  logicalBytes: number;
};

export type TelemetryRecordingMetadata = {
  schemaVersion: 1;
  id: string;
  status: "recording" | "complete";
  name: string;
  note: string;
  createdAtMs: number;
  completedAtMs: number | null;
  stopReason: RecordingStopReason | null;
  deviceIdHex: string;
  firmwareVersion: [number, number, number];
  vehicleProfileId: string;
  storageGeneration: number;
  transportIdentity: TransportIdentity;
  subscription: TelemetrySubscriptionSnapshot;
  channelDescriptors: TelemetryDescriptor[];
  parameterSnapshot: RecordingParameterSnapshot[];
  snapshotRevision: number;
  markers: string[];
  stats: TelemetryRecordingStats;
};

export type TelemetryRecordingChunk = {
  recordingId: string;
  chunkIndex: number;
  batches: UiTelemetryBatch[];
  logicalBytes: number;
};

export type TelemetryRecordingDocument = {
  format: typeof RECORDING_FORMAT;
  schemaVersion: 1;
  metadata: TelemetryRecordingMetadata;
  chunks: TelemetryRecordingChunk[];
};

type CreateRecordingMetadataInput = {
  id: string;
  name: string;
  note: string;
  snapshot: AppSnapshot;
  vehicleProfileId: string;
  createdAtMs: number;
};

const EMPTY_STATS: TelemetryRecordingStats = {
  batchCount: 0,
  pointCount: 0,
  droppedSamples: 0,
  firstTimestampUs: null,
  lastTimestampUs: null,
  chunkCount: 0,
  logicalBytes: 0,
};

export function recordingStartDenial(snapshot: AppSnapshot | null, name: string, note: string): string | null {
  const trimmedName = name.trim();
  const trimmedNote = note.trim();
  if (trimmedName.length === 0) return "记录名称不能为空";
  if (trimmedName.length > 64) return "记录名称最多 64 个字符";
  if (trimmedNote.length > 256) return "记录备注最多 256 个字符";
  if (snapshot?.phase !== "ready") return "设备就绪后才能开始录制";
  if (snapshot.activeSubscription === null) return "需要活动遥测订阅才能开始录制";
  if (snapshot.paused) return "波形暂停时不能开始录制";
  return null;
}

export function createRecordingMetadata(input: CreateRecordingMetadataInput): TelemetryRecordingMetadata {
  const denial = recordingStartDenial(input.snapshot, input.name, input.note);
  if (denial !== null) throw new Error(denial);
  assertUuid(input.id, "记录 ID");
  assertFiniteNonNegative(input.createdAtMs, "创建时间");
  const snapshot = input.snapshot;
  const subscription = snapshot.activeSubscription;
  if (subscription === null || snapshot.deviceIdHex === null || snapshot.firmwareVersion === null || snapshot.transportIdentity === null) {
    throw new Error("设备快照缺少录制所需身份信息");
  }
  const descriptorsById = new Map(snapshot.telemetryDescriptors.map((descriptor) => [descriptor.channelId, descriptor]));
  const channelDescriptors = subscription.channelIds.map((channelId) => {
    const descriptor = descriptorsById.get(channelId);
    if (descriptor === undefined) throw new Error(`订阅通道 ${channelId} 缺少描述符`);
    return { ...descriptor };
  });

  return {
    schemaVersion: RECORDING_SCHEMA_VERSION,
    id: input.id,
    status: "recording",
    name: input.name.trim(),
    note: input.note.trim(),
    createdAtMs: input.createdAtMs,
    completedAtMs: null,
    stopReason: null,
    deviceIdHex: snapshot.deviceIdHex,
    firmwareVersion: [...snapshot.firmwareVersion],
    vehicleProfileId: input.vehicleProfileId,
    storageGeneration: snapshot.storageGeneration,
    transportIdentity: cloneTransportIdentity(snapshot.transportIdentity),
    subscription: {
      channelIds: [...subscription.channelIds],
      sampleRateHz: subscription.sampleRateHz,
      subscriptionVersion: subscription.subscriptionVersion,
    },
    channelDescriptors,
    parameterSnapshot: snapshot.parameters.map(({ paramId, machineName, ramValue, revision }) => ({
      paramId,
      machineName,
      ramValue: { ...ramValue },
      revision,
    })),
    snapshotRevision: snapshot.revision,
    markers: [],
    stats: { ...EMPTY_STATS },
  };
}

export function createRecordingChunk(
  recordingId: string,
  chunkIndex: number,
  batches: readonly UiTelemetryBatch[],
): TelemetryRecordingChunk {
  assertUuid(recordingId, "记录 ID");
  if (!Number.isInteger(chunkIndex) || chunkIndex < 0) throw new Error("记录块序号无效");
  const clonedBatches = batches.map(cloneBatch);
  return {
    recordingId,
    chunkIndex,
    batches: clonedBatches,
    logicalBytes: chunkLogicalBytes(recordingId, chunkIndex, clonedBatches),
  };
}

export function completeRecordingMetadata(
  metadata: TelemetryRecordingMetadata,
  chunks: readonly TelemetryRecordingChunk[],
  stopReason: RecordingStopReason,
  completedAtMs: number,
  markers: readonly string[],
): TelemetryRecordingMetadata {
  if (metadata.status !== "recording") throw new Error("只能封存正在录制的记录");
  assertFiniteNonNegative(completedAtMs, "完成时间");
  const durationMs = completedAtMs - metadata.createdAtMs;
  if (durationMs < 0) throw new Error("完成时间不能早于创建时间");
  if (durationMs > MAX_RECORDING_DURATION_MS) throw new Error("单次录制不能超过 5 分钟");
  return {
    ...metadata,
    status: "complete",
    completedAtMs,
    stopReason,
    markers: [...markers],
    stats: calculateRecordingStats(chunks),
  };
}

export function calculateRecordingStats(chunks: readonly TelemetryRecordingChunk[]): TelemetryRecordingStats {
  let batchCount = 0;
  let pointCount = 0;
  let droppedSamples = 0;
  let firstTimestampUs: number | null = null;
  let lastTimestampUs: number | null = null;
  let logicalBytes = 0;
  for (const chunk of chunks) {
    logicalBytes += chunk.logicalBytes;
    for (const batch of chunk.batches) {
      batchCount += 1;
      pointCount += batch.points.length;
      droppedSamples += batch.droppedSamples;
      for (const point of batch.points) {
        firstTimestampUs ??= point.timestampUs;
        lastTimestampUs = point.timestampUs;
      }
    }
  }
  return {
    batchCount,
    pointCount,
    droppedSamples,
    firstTimestampUs,
    lastTimestampUs,
    chunkCount: chunks.length,
    logicalBytes,
  };
}

export function buildRecordingJsonBlob(document: TelemetryRecordingDocument): Blob {
  validateRecordingDocument(document);
  const parts: BlobPart[] = [
    `{"format":"${RECORDING_FORMAT}","schemaVersion":${RECORDING_SCHEMA_VERSION},"metadata":`,
    JSON.stringify(document.metadata),
    ",\"chunks\":[",
  ];
  document.chunks.forEach((chunk, index) => {
    if (index > 0) parts.push(",");
    parts.push(JSON.stringify(chunk));
  });
  parts.push("]}");
  return new Blob(parts, { type: "application/json;charset=utf-8" });
}

export function parseRecordingJson(text: string, fileSize = new TextEncoder().encode(text).byteLength): TelemetryRecordingDocument {
  if (fileSize > MAX_RECORDING_LIBRARY_BYTES) throw new Error("导入文件超过 256 MiB 上限");
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("记录 JSON 格式无效");
  }
  validateRecordingDocument(parsed);
  return parsed;
}

export function rekeyImportedRecording(
  document: TelemetryRecordingDocument,
  existingIds: ReadonlySet<string>,
  idFactory: () => string = () => crypto.randomUUID(),
): TelemetryRecordingDocument {
  validateRecordingDocument(document);
  if (!existingIds.has(document.metadata.id)) return cloneDocument(document);
  let newId = "";
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const candidate = idFactory();
    assertUuid(candidate, "新记录 ID");
    if (!existingIds.has(candidate)) {
      newId = candidate;
      break;
    }
  }
  if (newId.length === 0) throw new Error("无法为导入记录生成唯一 ID");
  const chunks = document.chunks.map((chunk) => createRecordingChunk(newId, chunk.chunkIndex, chunk.batches));
  return {
    ...cloneDocument(document),
    metadata: {
      ...document.metadata,
      id: newId,
      stats: calculateRecordingStats(chunks),
    },
    chunks,
  };
}

export function buildRecordingCsvBlob(document: TelemetryRecordingDocument): Blob {
  validateRecordingDocument(document);
  const descriptors = document.metadata.channelDescriptors;
  const header = [
    "batch_index",
    "subscription_version",
    "dropped_before",
    "timestamp_us",
    "sample_sequence",
    ...descriptors.map(({ machineName }) => machineName),
  ].map(csvCell).join(",");
  const parts: BlobPart[] = ["\ufeff", header, "\r\n"];
  let batchIndex = 0;
  for (const chunk of document.chunks) {
    for (const batch of chunk.batches) {
      const rows = new Map<string, { timestampUs: number; sampleSequence: number; values: Map<number, number> }>();
      for (const point of batch.points) {
        const key = `${point.timestampUs}:${point.sampleSequence}`;
        let row = rows.get(key);
        if (row === undefined) {
          row = { timestampUs: point.timestampUs, sampleSequence: point.sampleSequence, values: new Map() };
          rows.set(key, row);
        }
        row.values.set(point.channelId, point.value.value);
      }
      let rowIndex = 0;
      for (const row of rows.values()) {
        parts.push([
          batchIndex,
          batch.subscriptionVersion,
          rowIndex === 0 ? batch.droppedSamples : "",
          row.timestampUs,
          row.sampleSequence,
          ...descriptors.map(({ channelId }) => row.values.get(channelId) ?? ""),
        ].map(csvCell).join(","), "\r\n");
        rowIndex += 1;
      }
      batchIndex += 1;
    }
  }
  return new Blob(parts, { type: "text/csv;charset=utf-8" });
}

export function recordingFileName(name: string, extension: "json" | "csv"): string {
  const safe = name
    .normalize("NFKC")
    .replace(/^[\s.=+@/_\\-]+/u, "")
    .replace(/[^\p{L}\p{N}._-]+/gu, "-")
    .replace(/^[._-]+|[._-]+$/gu, "")
    .slice(0, 64) || "recording";
  return `dicar-recording-${safe}.${extension}`;
}

function validateRecordingDocument(value: unknown): asserts value is TelemetryRecordingDocument {
  const document = asRecord(value, "记录 JSON 格式无效");
  if (document.format !== RECORDING_FORMAT || document.schemaVersion !== RECORDING_SCHEMA_VERSION) {
    throw new Error("记录 JSON 格式或 schemaVersion 无效");
  }
  const metadata = asRecord(document.metadata, "记录元数据格式无效");
  assertUuid(metadata.id, "记录 ID");
  if (metadata.schemaVersion !== 1 || metadata.status !== "complete") throw new Error("只能导入完整的 schema v1 记录");
  validateNameAndNote(metadata.name, metadata.note);
  assertFiniteNonNegative(metadata.createdAtMs, "创建时间");
  assertFiniteNonNegative(metadata.completedAtMs, "完成时间");
  if ((metadata.completedAtMs as number) - (metadata.createdAtMs as number) > MAX_RECORDING_DURATION_MS) {
    throw new Error("导入记录超过 5 分钟上限");
  }
  if ((metadata.completedAtMs as number) < (metadata.createdAtMs as number)) throw new Error("记录完成时间无效");
  if (!isStopReason(metadata.stopReason)) throw new Error("记录停止原因无效");
  if (typeof metadata.deviceIdHex !== "string" || metadata.deviceIdHex.length === 0) throw new Error("设备 ID 无效");
  validateFirmware(metadata.firmwareVersion);
  if (typeof metadata.vehicleProfileId !== "string" || metadata.vehicleProfileId.length === 0) throw new Error("车型 ID 无效");
  assertInteger(metadata.storageGeneration, "Storage Generation");
  validateTransportIdentity(metadata.transportIdentity);
  const subscription = validateSubscription(metadata.subscription);
  const descriptors = validateDescriptors(metadata.channelDescriptors, subscription.channelIds);
  validateParameterSnapshots(metadata.parameterSnapshot);
  assertInteger(metadata.snapshotRevision, "Snapshot Revision");
  if (!Array.isArray(metadata.markers) || metadata.markers.some((marker) => typeof marker !== "string")) throw new Error("记录标记无效");

  if (!Array.isArray(document.chunks)) throw new Error("记录块格式无效");
  let previousTimestampUs = Number.NEGATIVE_INFINITY;
  for (let chunkIndex = 0; chunkIndex < document.chunks.length; chunkIndex += 1) {
    const chunk = asRecord(document.chunks[chunkIndex], "记录块格式无效");
    if (chunk.recordingId !== metadata.id || chunk.chunkIndex !== chunkIndex || !Array.isArray(chunk.batches)) {
      throw new Error("记录块 ID 或序号无效");
    }
    const expectedBytes = chunkLogicalBytes(chunk.recordingId, chunk.chunkIndex, chunk.batches as UiTelemetryBatch[]);
    if (chunk.logicalBytes !== expectedBytes) throw new Error("记录块逻辑字节统计无效");
    for (const batchValue of chunk.batches) {
      const batch = validateBatch(batchValue, subscription.subscriptionVersion, descriptors);
      for (const point of batch.points) {
        if (point.timestampUs < previousTimestampUs) throw new Error("记录点时间戳必须非递减");
        previousTimestampUs = point.timestampUs;
      }
    }
  }
  const expectedStats = calculateRecordingStats(document.chunks as TelemetryRecordingChunk[]);
  const actualStats = asRecord(metadata.stats, "记录统计格式无效");
  for (const key of Object.keys(expectedStats) as Array<keyof TelemetryRecordingStats>) {
    if (actualStats[key] !== expectedStats[key]) throw new Error(`记录统计 ${key} 无效`);
  }
}

function validateBatch(
  value: unknown,
  subscriptionVersion: number,
  descriptors: ReadonlyMap<number, TelemetryDescriptor>,
): UiTelemetryBatch {
  const batch = asRecord(value, "遥测批次格式无效");
  if (batch.subscriptionVersion !== subscriptionVersion) throw new Error("遥测批次订阅版本无效");
  assertUint16(batch.firstSampleSequence, "批次首序号");
  assertInteger(batch.droppedSamples, "批次丢样数");
  if ((batch.droppedSamples as number) < 0) throw new Error("批次丢样数无效");
  if (!Array.isArray(batch.points)) throw new Error("遥测批次点格式无效");
  for (const pointValue of batch.points) {
    const point = asRecord(pointValue, "遥测点格式无效");
    assertInteger(point.channelId, "遥测通道");
    const descriptor = descriptors.get(point.channelId as number);
    if (descriptor === undefined) throw new Error(`遥测点通道 ${String(point.channelId)} 未声明`);
    assertInteger(point.timestampUs, "遥测时间戳");
    if ((point.timestampUs as number) < 0) throw new Error("遥测时间戳无效");
    assertUint16(point.sampleSequence, "采样序号");
    validateTelemetryValue(point.value, descriptor.telemetryType);
  }
  if (batch.points.length > 0 && (batch.points[0] as { sampleSequence: number }).sampleSequence !== batch.firstSampleSequence) {
    throw new Error("批次首样本序号不一致");
  }
  return batch as unknown as UiTelemetryBatch;
}

function validateDescriptors(value: unknown, subscribedIds: readonly number[]): Map<number, TelemetryDescriptor> {
  if (!Array.isArray(value)) throw new Error("通道描述符格式无效");
  const byId = new Map<number, TelemetryDescriptor>();
  const machineNames = new Set<string>();
  for (const item of value) {
    const descriptor = asRecord(item, "通道描述符格式无效");
    assertInteger(descriptor.channelId, "通道 ID");
    if (!isTelemetryKind(descriptor.telemetryType)) throw new Error("通道遥测类型无效");
    for (const field of ["machineName", "displayName", "group", "unit"] as const) {
      if (typeof descriptor[field] !== "string") throw new Error(`通道 ${field} 无效`);
    }
    const channelId = descriptor.channelId as number;
    const machineName = descriptor.machineName as string;
    if (byId.has(channelId) || machineNames.has(machineName)) throw new Error("通道描述符必须唯一");
    byId.set(channelId, descriptor as unknown as TelemetryDescriptor);
    machineNames.add(machineName);
  }
  if (subscribedIds.length !== byId.size || subscribedIds.some((channelId) => !byId.has(channelId))) {
    throw new Error("订阅通道与通道描述符不一致");
  }
  return byId;
}

function validateSubscription(value: unknown): TelemetrySubscriptionSnapshot {
  const subscription = asRecord(value, "订阅格式无效");
  if (!Array.isArray(subscription.channelIds) || subscription.channelIds.length === 0) throw new Error("订阅通道无效");
  const ids = new Set<number>();
  for (const id of subscription.channelIds) {
    assertInteger(id, "订阅通道 ID");
    if (ids.has(id as number)) throw new Error("订阅通道不能重复");
    ids.add(id as number);
  }
  assertInteger(subscription.sampleRateHz, "订阅采样率");
  assertInteger(subscription.subscriptionVersion, "订阅版本");
  if ((subscription.sampleRateHz as number) <= 0 || (subscription.subscriptionVersion as number) < 0) throw new Error("订阅参数无效");
  return subscription as unknown as TelemetrySubscriptionSnapshot;
}

function validateParameterSnapshots(value: unknown): void {
  if (!Array.isArray(value)) throw new Error("参数快照格式无效");
  const ids = new Set<number>();
  for (const item of value) {
    const parameter = asRecord(item, "参数快照格式无效");
    assertInteger(parameter.paramId, "参数 ID");
    if (ids.has(parameter.paramId as number)) throw new Error("参数快照 ID 不能重复");
    ids.add(parameter.paramId as number);
    if (typeof parameter.machineName !== "string" || parameter.machineName.length === 0) throw new Error("参数 machine name 无效");
    assertInteger(parameter.revision, "参数 Revision");
    validateParameterValue(parameter.ramValue);
  }
}

function validateParameterValue(value: unknown): asserts value is ParameterValue {
  const parameterValue = asRecord(value, "参数值格式无效");
  switch (parameterValue.kind) {
    case "f32":
      if (typeof parameterValue.value !== "number" || !Number.isFinite(parameterValue.value)) throw new Error("f32 参数值无效");
      return;
    case "i32":
    case "enum":
      assertInt32(parameterValue.value, "i32 参数值");
      return;
    case "u32":
      assertUint32(parameterValue.value, "u32 参数值");
      return;
    case "bool":
      if (typeof parameterValue.value !== "boolean") throw new Error("bool 参数值无效");
      return;
    default:
      throw new Error("参数值类型无效");
  }
}

function validateTelemetryValue(value: unknown, expectedKind: TelemetryValue["kind"]): void {
  const telemetryValue = asRecord(value, "遥测值格式无效");
  if (telemetryValue.kind !== expectedKind) throw new Error("遥测值类型与通道描述不一致");
  if (expectedKind === "f32") {
    if (typeof telemetryValue.value !== "number" || !Number.isFinite(telemetryValue.value)) throw new Error("f32 遥测值无效");
  } else if (expectedKind === "i32") assertInt32(telemetryValue.value, "i32 遥测值");
  else assertUint32(telemetryValue.value, "u32 遥测值");
}

function validateNameAndNote(name: unknown, note: unknown): void {
  if (typeof name !== "string" || name.trim().length === 0 || name !== name.trim() || name.length > 64) throw new Error("记录名称无效");
  if (typeof note !== "string" || note !== note.trim() || note.length > 256) throw new Error("记录备注无效");
}

function validateFirmware(value: unknown): asserts value is [number, number, number] {
  if (!Array.isArray(value) || value.length !== 3) throw new Error("固件版本无效");
  value.forEach((part) => assertInteger(part, "固件版本"));
}

function validateTransportIdentity(value: unknown): void {
  const identity = asRecord(value, "连接标识无效");
  const endpoint = asRecord(identity.endpoint, "连接端点无效");
  if (endpoint.kind === "simulator") {
    if (typeof endpoint.address !== "string" || endpoint.address.length === 0) throw new Error("模拟器端点无效");
  } else if (endpoint.kind === "serial") {
    if (typeof endpoint.portName !== "string" || typeof endpoint.baudRate !== "number" || typeof endpoint.hardwareProfile !== "string") throw new Error("串口端点无效");
  } else throw new Error("连接端点类型无效");
}

function cloneBatch(batch: UiTelemetryBatch): UiTelemetryBatch {
  return {
    subscriptionVersion: batch.subscriptionVersion,
    firstSampleSequence: batch.firstSampleSequence,
    droppedSamples: batch.droppedSamples,
    points: batch.points.map((point) => ({ ...point, value: { ...point.value } })),
  };
}

function cloneTransportIdentity(identity: TransportIdentity): TransportIdentity {
  return {
    endpoint: identity.endpoint.kind === "simulator"
      ? { ...identity.endpoint }
      : { ...identity.endpoint },
  };
}

function cloneDocument(document: TelemetryRecordingDocument): TelemetryRecordingDocument {
  return structuredClone(document);
}

function chunkLogicalBytes(recordingId: unknown, chunkIndex: unknown, batches: unknown): number {
  return new TextEncoder().encode(JSON.stringify({ recordingId, chunkIndex, batches })).byteLength;
}

function csvCell(value: string | number): string {
  let text = String(value);
  if (/^[=+\-@]/u.test(text)) text = `'${text}`;
  return /[",\r\n]/u.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function asRecord(value: unknown, message: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error(message);
  return value as Record<string, unknown>;
}

function assertUuid(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string" || !/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu.test(value)) {
    throw new Error(`${label} 无效`);
  }
}

function assertFiniteNonNegative(value: unknown, label: string): asserts value is number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) throw new Error(`${label} 无效`);
}

function assertInteger(value: unknown, label: string): asserts value is number {
  if (typeof value !== "number" || !Number.isSafeInteger(value)) throw new Error(`${label} 无效`);
}

function assertUint16(value: unknown, label: string): asserts value is number {
  assertInteger(value, label);
  if (value < 0 || value > 0xffff) throw new Error(`${label} 无效`);
}

function assertInt32(value: unknown, label: string): asserts value is number {
  assertInteger(value, label);
  if (value < -0x8000_0000 || value > 0x7fff_ffff) throw new Error(`${label} 无效`);
}

function assertUint32(value: unknown, label: string): asserts value is number {
  assertInteger(value, label);
  if (value < 0 || value > 0xffff_ffff) throw new Error(`${label} 无效`);
}

function isTelemetryKind(value: unknown): value is TelemetryValue["kind"] {
  return value === "f32" || value === "i32" || value === "u32" || value === "flags32";
}

function isStopReason(value: unknown): value is RecordingStopReason {
  return value === "manual"
    || value === "durationLimit"
    || value === "paused"
    || value === "connectionLost"
    || value === "subscriptionChanged";
}
