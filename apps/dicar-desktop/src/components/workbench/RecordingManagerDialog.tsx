import { DownloadSimple, Play, Trash, UploadSimple, X } from "@phosphor-icons/react";
import { useEffect, useState } from "react";

import { useRecordingController } from "../../app/providers";
import {
  buildRecordingCsvBlob,
  buildRecordingJsonBlob,
  recordingFileName,
  type TelemetryRecordingMetadata,
} from "../../telemetry/recordings";
import { Button } from "../ui/button";

export type RecordingDownload = (blob: Blob, fileName: string) => void;

type Props = {
  open: boolean;
  onClose: () => void;
  onReplay: (recordingId: string) => void;
  download?: RecordingDownload;
};

export function RecordingManagerDialog({ open, onClose, onReplay, download = downloadBlob }: Props) {
  const controller = useRecordingController();
  const [recordings, setRecordings] = useState<TelemetryRecordingMetadata[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void controller.listRecordings().then((items) => {
      if (!cancelled) setRecordings(items);
    }).catch(() => {
      if (!cancelled) setMessage("无法读取波形记录库");
    });
    return () => { cancelled = true; };
  }, [controller, open]);

  if (!open) return null;

  async function refresh(): Promise<void> {
    setRecordings(await controller.listRecordings());
  }

  async function remove(recording: TelemetryRecordingMetadata): Promise<void> {
    setBusyId(recording.id);
    try {
      await controller.deleteRecording(recording.id);
      await refresh();
      setMessage(`已删除「${recording.name}」`);
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      setBusyId(null);
    }
  }

  async function exportRecording(recording: TelemetryRecordingMetadata, format: "json" | "csv"): Promise<void> {
    const release = controller.protect(recording.id);
    setBusyId(recording.id);
    try {
      const document = await controller.getDocument(recording.id);
      if (document === null) throw new Error("记录不存在或尚未封存");
      const blob = format === "json" ? buildRecordingJsonBlob(document) : buildRecordingCsvBlob(document);
      download(blob, recordingFileName(recording.name, format));
      setMessage(`已导出「${recording.name}」${format.toUpperCase()}`);
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      release();
      setBusyId(null);
    }
  }

  async function importFile(file: File): Promise<void> {
    setBusyId("import");
    try {
      const text = await readFileText(file);
      await controller.importJson(text, file.size);
      await refresh();
      setMessage("记录导入成功");
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      setBusyId(null);
    }
  }

  return <div className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
    <section aria-labelledby="recording-manager-title" aria-modal="true" className="max-h-[85vh] w-full max-w-4xl overflow-auto rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) shadow-2xl" role="dialog">
      <header className="flex items-start justify-between gap-3 border-b border-(--border) p-4"><div><h2 className="m-0 text-base" id="recording-manager-title">波形记录库</h2><p className="m-0 mt-1 text-xs text-(--text-muted)">最多 20 条 / 256 MiB；达到上限时自动清理最旧完整记录。</p></div><Button aria-label="关闭波形记录库" onClick={onClose} size="sm" variant="secondary"><X size={15} /></Button></header>
      <div className="space-y-4 p-4">
        <label className="inline-flex cursor-pointer items-center gap-2 rounded-[var(--radius)] border border-(--border) px-3 py-2 text-xs font-semibold text-(--text)"><UploadSimple size={15} />导入 JSON<input accept="application/json,.json" aria-label="导入记录 JSON" className="sr-only" disabled={busyId !== null} onChange={(event) => { const file = event.currentTarget.files?.[0]; event.currentTarget.value = ""; if (file) void importFile(file); }} type="file" /></label>
        {message !== null && <p aria-live="polite" className="m-0 text-xs text-(--text-muted)">{message}</p>}
        {recordings.length === 0 ? <p className="m-0 rounded border border-dashed border-(--border) p-5 text-center text-xs text-(--text-muted)">还没有完整波形记录。</p> : <ul className="m-0 list-none space-y-2 p-0">{recordings.map((recording) => <li className="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius)] border border-(--border) p-3" data-testid="recording-row" key={recording.id}><div className="min-w-0"><p className="m-0 truncate text-sm font-medium">{recording.name}</p><p className="m-0 mt-1 text-[11px] text-(--text-muted)">{new Date(recording.createdAtMs).toLocaleString("zh-CN")} · {recording.stats.pointCount.toLocaleString("zh-CN")} 点 · {formatBytes(recording.stats.logicalBytes)} · {stopReasonLabel(recording.stopReason)}</p>{recording.note.length > 0 && <p className="m-0 mt-1 truncate text-[11px] text-(--text-muted)">{recording.note}</p>}</div><div className="flex flex-wrap gap-1.5"><Button aria-label={`回放 ${recording.name}`} disabled={busyId !== null} onClick={() => onReplay(recording.id)} size="sm" variant="secondary"><Play size={14} />回放</Button><Button aria-label={`导出 JSON ${recording.name}`} disabled={busyId !== null} onClick={() => void exportRecording(recording, "json")} size="sm" variant="secondary"><DownloadSimple size={14} />JSON</Button><Button aria-label={`导出 CSV ${recording.name}`} disabled={busyId !== null} onClick={() => void exportRecording(recording, "csv")} size="sm" variant="secondary"><DownloadSimple size={14} />CSV</Button><Button aria-label={`删除 ${recording.name}`} disabled={busyId !== null} onClick={() => void remove(recording)} size="sm" variant="danger"><Trash size={14} /></Button></div></li>)}</ul>}
      </div>
    </section>
  </div>;
}

export function downloadBlob(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  URL.revokeObjectURL(url);
}

function readFileText(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("无法读取导入文件"));
    reader.onload = () => resolve(String(reader.result));
    reader.readAsText(file);
  });
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : "波形记录操作失败";
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function stopReasonLabel(reason: TelemetryRecordingMetadata["stopReason"]): string {
  switch (reason) {
    case "manual": return "手动停止";
    case "durationLimit": return "5 分钟到期";
    case "paused": return "波形暂停";
    case "connectionLost": return "连接中断";
    case "subscriptionChanged": return "订阅变化";
    default: return "已封存";
  }
}
