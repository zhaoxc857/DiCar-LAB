import { Database, HardDrives, ShieldCheck } from "@phosphor-icons/react";
import { useState } from "react";
import { RecordingLibrary } from "../components/workbench/RecordingLibrary";
import { RecordingPlaybackDialog } from "../components/workbench/RecordingPlaybackDialog";

export function RecordingsPage() {
  const [playbackRecordingId, setPlaybackRecordingId] = useState<string | null>(null);

  return (
    <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <p className="m-0 text-xs font-semibold tracking-[0.14em] text-(--interactive)">遥测档案</p>
          <h1 className="mb-0 mt-1 text-2xl font-semibold">波形记录</h1>
          <p className="mb-0 mt-2 text-sm text-(--text-muted)">
            管理完整原始批次，并在独立只读缓冲中回放。
          </p>
        </div>
        <div className="grid grid-cols-2 gap-2 text-[11px] text-(--text-muted)">
          <span className="app-surface inline-flex items-center gap-2 px-3 py-2">
            <HardDrives aria-hidden="true" className="text-(--interactive)" size={16} />原始批次
          </span>
          <span className="app-surface inline-flex items-center gap-2 px-3 py-2">
            <ShieldCheck aria-hidden="true" className="text-(--success)" size={16} />只读回放
          </span>
        </div>
      </header>

      <section className="app-surface mt-5 p-4 lg:p-5" aria-labelledby="recording-library-title">
        <div className="mb-4 flex items-center gap-2">
          <Database aria-hidden="true" className="text-(--interactive)" size={20} />
          <h2 className="m-0 text-base font-semibold" id="recording-library-title">记录库</h2>
        </div>
        <RecordingLibrary onReplay={setPlaybackRecordingId} />
      </section>

      <RecordingPlaybackDialog
        onClose={() => setPlaybackRecordingId(null)}
        open={playbackRecordingId !== null}
        recordingId={playbackRecordingId}
      />
    </main>
  );
}
