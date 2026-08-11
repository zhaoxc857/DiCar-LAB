import type { TelemetryDescriptor } from "../../domain/types";

const colors = ["#38bdf8", "#34d399", "#fbbf24", "#fb7185", "#a78bfa", "#22d3ee", "#f472b6", "#a3e635"];

export function WaveformPreviewPanel({ descriptors }: { descriptors: TelemetryDescriptor[] }) {
  const active = descriptors.slice(0, 8);
  return <section className="min-w-0 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3">
    <header className="flex items-start justify-between gap-3"><div><h2 className="m-0 text-sm">实时波形</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">最多 8 路 · 500 Hz · 10 s 窗口</p></div><span className="rounded border border-(--success) px-2 py-1 text-[10px] text-(--success)">{active.length}/8 通道</span></header>
    <div className="mt-3 overflow-hidden rounded border border-(--border) bg-(--background)"><svg aria-label="实时波形预览" className="block h-56 w-full" role="img" viewBox="0 0 600 240"><defs><pattern height="24" id="grid" patternUnits="userSpaceOnUse" width="40"><path d="M 40 0 L 0 0 0 24" fill="none" stroke="currentColor" strokeOpacity="0.08" /></pattern></defs><rect fill="url(#grid)" height="240" width="600" />{active.map((channel, index) => <path d={wavePath(index)} fill="none" key={channel.channelId} stroke={colors[index]} strokeWidth="1.6" />)}</svg></div>
    <div className="mt-3 grid gap-1.5 sm:grid-cols-2">{active.map((channel, index) => <div className="flex min-w-0 items-center gap-2 text-[10px]" key={channel.channelId}><span className="size-2 shrink-0 rounded-full" style={{ background: colors[index] }} /><span className="truncate">{channel.displayName}</span><span className="ml-auto font-mono text-(--text-muted)">{channel.unit}</span></div>)}</div>
    <p className="m-0 mt-3 border-t border-(--border) pt-3 text-[10px] text-(--text-muted)">首个可见纵向切片使用确定性预览；下一阶段接入有界环形缓冲、降采样、暂停与游标。</p>
  </section>;
}

function wavePath(index: number): string {
  const amplitude = 16 + index * 2;
  const center = 28 + index * 25;
  return Array.from({ length: 61 }, (_, point) => `${point === 0 ? "M" : "L"}${point * 10},${center + Math.sin(point / (3 + index * 0.35)) * amplitude}`).join(" ");
}
