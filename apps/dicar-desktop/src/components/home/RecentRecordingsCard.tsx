import { ClockCounterClockwise } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import { Link } from "react-router";
import { useRecordingController } from "../../app/providers";
import type { TelemetryRecordingMetadata } from "../../telemetry/recordings";
import { Card } from "../ui/card";

export function RecentRecordingsCard({ limit = 3 }: { limit?: number }) {
  const controller = useRecordingController();
  const [recordings, setRecordings] = useState<TelemetryRecordingMetadata[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void controller.listRecordings().then((items) => {
      if (!cancelled) setRecordings(items.slice(0, limit));
    }).catch(() => {
      if (!cancelled) setError("无法读取最近波形记录");
    });
    return () => {
      cancelled = true;
    };
  }, [controller, limit]);

  return (
    <Card className="h-full p-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="m-0 text-sm font-semibold">最近记录</h2>
          <p className="m-0 mt-1 text-xs text-(--text-muted)">最近封存的完整波形</p>
        </div>
        <ClockCounterClockwise aria-hidden="true" className="text-(--interactive)" size={22} />
      </div>

      {error !== null ? (
        <p className="mt-4 text-xs text-(--danger)" role="status">{error}</p>
      ) : recordings.length === 0 ? (
        <p className="mt-4 rounded-[var(--radius)] border border-dashed border-(--border) p-4 text-center text-xs text-(--text-muted)">
          还没有完整波形记录。
        </p>
      ) : (
        <ul className="m-0 mt-3 list-none space-y-2 p-0">
          {recordings.map((recording) => (
            <li className="rounded-[var(--radius)] border border-(--border-subtle) bg-(--surface) p-3" key={recording.id}>
              <strong className="block truncate text-xs">{recording.name}</strong>
              <span className="data-value mt-1 block text-[10px] text-(--text-muted)">
                {recording.stats.pointCount.toLocaleString("zh-CN")} 点 · {new Date(recording.createdAtMs).toLocaleString("zh-CN")}
              </span>
            </li>
          ))}
        </ul>
      )}

      <Link className="mt-4 inline-flex text-xs font-semibold text-(--interactive)" to="/records">
        查看全部记录
      </Link>
    </Card>
  );
}
