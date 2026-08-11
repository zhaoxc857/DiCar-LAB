import { ArrowRight, WaveSine } from "@phosphor-icons/react";
import { useConnectionStore } from "../stores/connectionStore";

export function LivePreviewPage() {
  const snapshot = useConnectionStore((state) => state.snapshot);
  return <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content"><div className="flex items-center gap-3"><WaveSine className="text-(--interactive)" size={28} /><div><h1 className="m-0 text-xl">实时调参与波形</h1><p className="m-0 mt-1 text-xs text-(--text-muted)">车辆 CAR-01 · {snapshot?.parameters.length ?? 0} 个参数 · {snapshot?.telemetryDescriptors.length ?? 0} 个通道</p></div></div><div className="mt-5 grid min-h-[480px] gap-3 lg:grid-cols-[240px_minmax(360px,1fr)_minmax(360px,1.2fr)]"><Preview title="参数导航" text="速度环 PID / 编码器 / 车辆模型" /><Preview title="参数编辑器" text="RAM、Flash、Revision 与权限控制" /><Preview title="实时波形" text="最多 8 路通道 · 500 Hz · Canvas" /></div></main>;
}
function Preview({ title, text }: { title: string; text: string }) { return <section className="rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-4"><h2 className="m-0 text-sm">{title}</h2><p className="mt-2 text-xs text-(--text-muted)">{text}</p><span className="mt-6 inline-flex items-center gap-1 text-xs text-(--interactive)">工作台实现中 <ArrowRight size={13} /></span></section>; }
