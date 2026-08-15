import { DownloadSimple, Play, Trash, UploadSimple } from "@phosphor-icons/react";
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

export type RecordingLibraryProps = {
  onReplay: (recordingId: string) => void;
  download?: RecordingDownload;
};

export function RecordingLibrary({
  onReplay,
  download = downloadBlob,
}: RecordingLibraryProps) {
  const controller = useRecordingController();
  const [recordings, setRecordings] = useState<TelemetryRecordingMetadata[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void controller.listRecordings().then((items) => {
      if (!cancelled) setRecordings(items);
    }).catch(() => {
      if (!cancelled) setMessage("无法读取波形记录库");
    });
    return () => {
      cancelled = true;
    };
  }, [controller]);

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

  async function exportRecording(
    recording: TelemetryRecordingMetadata,
    format: "json" | "csv",
  ): Promise<void> {
    const release = controller.protect(recording.id);
    setBusyId(recording.id);
    try {
      const document = await controller.getDocument(recording.id);
      if (document === null) throw new Error("记录不存在或尚未封存");
      const blob = format === "json"
        ? buildRecordingJsonBlob(document)
        : buildRecordingCsvBlob(document);
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

  return (
    <section aria-label="波形记录库内容" className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <label className="inline-flex min-h-10 cursor-pointer items-center gap-2 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) px-3 text-xs font-semibold text-(--text) hover:border-(--interactive)">
          <UploadSimple aria-hidden="true" size={15} />
          导入 JSON
          <input
            accept="application/json,.json"
            aria-label="导入记录 JSON"
            className="sr-only"
            disabled={busyId !== null}
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (file) void importFile(file);
            }}
            type="file"
          />
        </label>
        <p className="m-0 text-[11px] text-(--text-muted)">
          最多 20 条 / 256 MiB · 超限时清理最旧完整记录
        </p>
      </div>

      {message !== null && (
        <p aria-live="polite" className="m-0 text-xs text-(--text-muted)">{message}</p>
      )}

      {recordings.length === 0 ? (
        <p className="m-0 rounded-[var(--radius-lg)] border border-dashed border-(--border) p-8 text-center text-sm text-(--text-muted)">
          还没有完整波形记录。
        </p>
      ) : (
        <ul className="m-0 list-none space-y-2 p-0">
          {recordings.map((recording) => (
            <li
              className="app-surface flex flex-wrap items-center justify-between gap-4 p-4 hover:border-(--interactive)"
              data-testid="recording-row"
              key={recording.id}
            >
              <div className="min-w-0 flex-1">
                <p className="m-0 truncate text-sm font-semibold">{recording.name}</p>
                <p className="data-value m-0 mt-1 text-[11px] text-(--text-muted)">
                  {new Date(recording.createdAtMs).toLocaleString("zh-CN")} · {recording.stats.pointCount.toLocaleString("zh-CN")} 点 · {formatBytes(recording.stats.logicalBytes)} · {stopReasonLabel(recording.stopReason)}
                </p>
                {recording.note.length > 0 && (
                  <p className="m-0 mt-1 truncate text-[11px] text-(--text-muted)">{recording.note}</p>
                )}
              </div>
              <div className="flex flex-wrap gap-1.5">
                <Button
                  aria-label={`回放 ${recording.name}`}
                  disabled={busyId !== null}
                  onClick={() => onReplay(recording.id)}
                  size="sm"
                  variant="secondary"
                >
                  <Play aria-hidden="true" size={14} />回放
                </Button>
                <Button
                  aria-label={`导出 JSON ${recording.name}`}
                  disabled={busyId !== null}
                  onClick={() => void exportRecording(recording, "json")}
                  size="sm"
                  variant="secondary"
                >
                  <DownloadSimple aria-hidden="true" size={14} />JSON
                </Button>
                <Button
                  aria-label={`导出 CSV ${recording.name}`}
                  disabled={busyId !== null}
                  onClick={() => void exportRecording(recording, "csv")}
                  size="sm"
                  variant="secondary"
                >
                  <DownloadSimple aria-hidden="true" size={14} />CSV
                </Button>
                <Button
                  aria-label={`删除 ${recording.name}`}
                  disabled={busyId !== null}
                  onClick={() => void remove(recording)}
                  size="sm"
                  variant="danger"
                >
                  <Trash aria-hidden="true" size={14} />
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
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
