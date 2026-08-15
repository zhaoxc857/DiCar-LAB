import { X } from "@phosphor-icons/react";
import { Button } from "../ui/button";
import {
  RecordingLibrary,
  type RecordingDownload,
} from "./RecordingLibrary";

export type { RecordingDownload } from "./RecordingLibrary";

type Props = {
  open: boolean;
  onClose: () => void;
  onReplay: (recordingId: string) => void;
  download?: RecordingDownload;
};

export function RecordingManagerDialog({
  open,
  onClose,
  onReplay,
  download,
}: Props) {
  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/70 p-4"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        aria-labelledby="recording-manager-title"
        aria-modal="true"
        className="max-h-[85vh] w-full max-w-4xl overflow-auto rounded-[var(--radius-lg)] border border-(--border) bg-(--surface-raised) shadow-2xl"
        role="dialog"
      >
        <header className="flex items-start justify-between gap-3 border-b border-(--border) p-4">
          <div>
            <h2 className="m-0 text-base" id="recording-manager-title">波形记录库</h2>
            <p className="m-0 mt-1 text-xs text-(--text-muted)">
              完整原始批次、独立回放与安全导入导出。
            </p>
          </div>
          <Button aria-label="关闭波形记录库" onClick={onClose} size="sm" variant="secondary">
            <X aria-hidden="true" size={15} />
          </Button>
        </header>
        <div className="p-4">
          <RecordingLibrary download={download} onReplay={onReplay} />
        </div>
      </section>
    </div>
  );
}
