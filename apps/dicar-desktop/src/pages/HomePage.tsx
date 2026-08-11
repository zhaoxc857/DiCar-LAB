import { Database, Pulse, SlidersHorizontal, WaveSine } from "@phosphor-icons/react";
import { MenuCard } from "../components/home/MenuCard";
import { ProjectSummary } from "../components/home/ProjectSummary";

export function HomePage() {
  return (
    <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content">
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div><p className="m-0 text-xs font-semibold uppercase tracking-[0.16em] text-(--interactive)">Operations console</p><h1 className="mb-0 mt-1 text-2xl font-semibold">工作区</h1><p className="mb-0 mt-2 text-sm text-(--text-muted)">从车辆连接到实时调参，所有操作都保留设备确认状态。</p></div>
        <span className="font-mono text-xs text-(--text-muted)">DCTP v1 · SIMULATOR READY</span>
      </div>
      <section aria-label="应用目的地" className="grid gap-3 md:grid-cols-2">
        <MenuCard description="实时编辑 RAM 参数、观察编码器和控制环波形，并审阅固化到 Flash 的修改。" icon={WaveSine} status="可用" title="实时调参与波形" to="/live/car-01" />
        <MenuCard description="记录设备会话、标记关键时刻并离线回放遥测；首版后续阶段开放。" icon={Database} status="计划发布" title="数据记录与回放" to="/records" />
        <MenuCard description="比较、评审并应用可追踪的参数方案；首版后续阶段开放。" icon={SlidersHorizontal} status="计划发布" title="参数方案库" to="/parameter-sets" />
        <MenuCard description="检查会话、链路流量、CRC、丢样和前端背压等实时指标。" icon={Pulse} status="可用" title="连接与链路诊断" to="/diagnostics" />
      </section>
      <section className="mt-4" aria-label="项目摘要"><ProjectSummary /></section>
    </main>
  );
}
