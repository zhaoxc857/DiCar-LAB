import { Database, Pulse, SlidersHorizontal } from "@phosphor-icons/react";
import { MenuCard } from "../components/home/MenuCard";
import { ProjectSummary } from "../components/home/ProjectSummary";
import { RecentRecordingsCard } from "../components/home/RecentRecordingsCard";

export function HomePage() {
  return (
    <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content">
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div><p className="m-0 text-xs font-semibold tracking-[0.16em] text-(--interactive)">精准控制台</p><h1 className="mb-0 mt-1 text-2xl font-semibold">概览</h1><p className="mb-0 mt-2 text-sm text-(--text-muted)">设备真值、当前车型与最近遥测记录集中呈现。</p></div>
        <span className="data-value text-xs text-(--text-muted)">DCTP v1 · RAM / FLASH 独立确认</span>
      </div>
      <section aria-label="应用目的地" className="grid gap-3 md:grid-cols-3">
        <MenuCard actionLabel="进入实时调试" description="编辑 RAM 参数、观察控制环波形，并审阅需要固化的修改。" icon={SlidersHorizontal} title="实时调试" to="/live" />
        <MenuCard actionLabel="打开波形记录" description="管理完整原始遥测批次，导入导出并在独立缓冲中回放。" icon={Database} title="波形记录" to="/records" />
        <MenuCard actionLabel="查看诊断" description="检查会话、链路流量、CRC、丢样和前端背压等实时指标。" icon={Pulse} title="诊断" to="/diagnostics" />
      </section>
      <section aria-label="车辆与记录摘要" className="mt-4 grid gap-3 lg:grid-cols-[1.4fr_1fr]">
        <ProjectSummary />
        <RecentRecordingsCard />
      </section>
    </main>
  );
}
