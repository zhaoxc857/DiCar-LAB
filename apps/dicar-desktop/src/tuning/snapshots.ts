import type { AppSnapshot, ParameterSnapshot, ParameterValue } from "../domain/types";

/** 一份可恢复的参数方案（规格 §14 的"参数快照"）。 */
export interface TuningSnapshotEntry {
  paramId: number;
  machineName: string;
  displayName: string;
  unit: string;
  value: ParameterValue;
}

export type TuningSnapshotOrigin = "manual" | "commit";

export interface TuningSnapshot {
  id: string;
  name: string;
  note: string;
  createdAtMs: number;
  origin: TuningSnapshotOrigin;
  deviceIdHex: string | null;
  firmwareVersion: [number, number, number] | null;
  /** origin 为 commit 时记录固化后的设备 Generation。 */
  storageGeneration: number | null;
  profileId: string;
  entries: TuningSnapshotEntry[];
}

export interface CaptureMeta {
  name: string;
  note: string;
  origin: TuningSnapshotOrigin;
  profileId: string;
  nowMs: number;
  id: string;
}

/** 从当前设备快照捕获全部 RAM 运行值。设备未就绪（无参数）时返回 null。 */
export function captureTuningSnapshot(snapshot: AppSnapshot, meta: CaptureMeta): TuningSnapshot | null {
  if (snapshot.parameters.length === 0) return null;
  return {
    id: meta.id,
    name: meta.name,
    note: meta.note,
    createdAtMs: meta.nowMs,
    origin: meta.origin,
    deviceIdHex: snapshot.deviceIdHex,
    firmwareVersion: snapshot.firmwareVersion,
    storageGeneration: meta.origin === "commit" ? snapshot.storageGeneration : null,
    profileId: meta.profileId,
    entries: snapshot.parameters.map((record) => ({
      paramId: record.paramId,
      machineName: record.machineName,
      displayName: record.displayName,
      unit: record.unit,
      value: record.ramValue,
    })),
  };
}

/**
 * 方案条目与当前设备状态的比对结果。规格 §12.3：缺失、类型变化和越界
 * 项必须明确列出且不得自动应用；只读与设备状态未知同样只能跳过。
 */
export type EntryDisposition =
  | "apply"
  | "match"
  | "missingParam"
  | "typeChanged"
  | "outOfRange"
  | "readOnly"
  | "unknownState";

export interface SnapshotDiffEntry {
  entry: TuningSnapshotEntry;
  disposition: EntryDisposition;
  currentValue: ParameterValue | null;
}

export interface SnapshotDiff {
  entries: SnapshotDiffEntry[];
  applicable: SnapshotDiffEntry[];
  blocked: SnapshotDiffEntry[];
}

export function diffTuningSnapshot(snapshot: TuningSnapshot, parameters: ParameterSnapshot[]): SnapshotDiff {
  const entries = snapshot.entries.map((entry): SnapshotDiffEntry => {
    const record = parameters.find(({ paramId }) => paramId === entry.paramId);
    if (record === undefined) return { entry, disposition: "missingParam", currentValue: null };
    const current = record.ramValue;
    if (current.kind !== entry.value.kind) return { entry, disposition: "typeChanged", currentValue: current };
    if (valuesEqual(entry.value, current)) return { entry, disposition: "match", currentValue: current };
    if (!record.syncKnown) return { entry, disposition: "unknownState", currentValue: current };
    if (!record.writable) return { entry, disposition: "readOnly", currentValue: current };
    if (!withinConstraints(entry.value, record)) return { entry, disposition: "outOfRange", currentValue: current };
    return { entry, disposition: "apply", currentValue: current };
  });
  return {
    entries,
    applicable: entries.filter(({ disposition }) => disposition === "apply"),
    blocked: entries.filter(({ disposition }) => disposition !== "apply" && disposition !== "match"),
  };
}

export function valuesEqual(left: ParameterValue, right: ParameterValue): boolean {
  return left.kind === right.kind && left.value === right.value;
}

function withinConstraints(value: ParameterValue, record: ParameterSnapshot): boolean {
  if (value.kind === "enum") {
    const options = record.enumOptions ?? [];
    return options.some((option) => option.value === value.value);
  }
  if (value.kind === "bool") return true;
  if (!Number.isFinite(value.value)) return false;
  if (record.numeric === undefined) return true;
  return value.value >= record.numeric.min && value.value <= record.numeric.max;
}

export const DISPOSITION_LABELS: Record<EntryDisposition, string> = {
  apply: "将写入 RAM",
  match: "与当前一致",
  missingParam: "设备缺少该参数",
  typeChanged: "参数类型已变化",
  outOfRange: "超出当前允许范围",
  readOnly: "参数为只读",
  unknownState: "设备状态未知",
};

/** 导出为可提交到 Git 仓库的可读 JSON（规格 §14，由用户显式触发）。 */
export function snapshotExportJson(snapshot: TuningSnapshot): string {
  return `${JSON.stringify({ format: "dicar-tuning-snapshot", version: 1, snapshot }, null, 2)}\n`;
}

/** 持久化读取时的结构校验；损坏条目返回 null 由调用方丢弃。 */
export function parseStoredSnapshot(raw: unknown): TuningSnapshot | null {
  if (typeof raw !== "object" || raw === null) return null;
  const value = raw as Partial<TuningSnapshot>;
  if (
    typeof value.id !== "string" ||
    value.id.length === 0 ||
    typeof value.name !== "string" ||
    typeof value.note !== "string" ||
    typeof value.createdAtMs !== "number" ||
    (value.origin !== "manual" && value.origin !== "commit") ||
    typeof value.profileId !== "string" ||
    !Array.isArray(value.entries)
  ) {
    return null;
  }
  const entries: TuningSnapshotEntry[] = [];
  for (const entry of value.entries) {
    const parsed = parseStoredEntry(entry);
    if (parsed === null) return null;
    entries.push(parsed);
  }
  const firmware = value.firmwareVersion;
  return {
    id: value.id,
    name: value.name,
    note: value.note,
    createdAtMs: value.createdAtMs,
    origin: value.origin,
    deviceIdHex: typeof value.deviceIdHex === "string" ? value.deviceIdHex : null,
    firmwareVersion:
      Array.isArray(firmware) && firmware.length === 3 && firmware.every((part) => typeof part === "number")
        ? [firmware[0], firmware[1], firmware[2]]
        : null,
    storageGeneration: typeof value.storageGeneration === "number" ? value.storageGeneration : null,
    profileId: value.profileId,
    entries,
  };
}

function parseStoredEntry(raw: unknown): TuningSnapshotEntry | null {
  if (typeof raw !== "object" || raw === null) return null;
  const value = raw as Partial<TuningSnapshotEntry>;
  if (
    typeof value.paramId !== "number" ||
    typeof value.machineName !== "string" ||
    typeof value.displayName !== "string" ||
    typeof value.unit !== "string" ||
    !isParameterValue(value.value)
  ) {
    return null;
  }
  return {
    paramId: value.paramId,
    machineName: value.machineName,
    displayName: value.displayName,
    unit: value.unit,
    value: value.value,
  };
}

function isParameterValue(raw: unknown): raw is ParameterValue {
  if (typeof raw !== "object" || raw === null) return false;
  const value = raw as { kind?: unknown; value?: unknown };
  if (value.kind === "bool") return typeof value.value === "boolean";
  if (value.kind === "f32" || value.kind === "i32" || value.kind === "u32" || value.kind === "enum") {
    return typeof value.value === "number";
  }
  return false;
}
