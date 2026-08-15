import { BookmarksSimple, Database, Pulse, WaveSine } from "@phosphor-icons/react";
import { MenuCard } from "../components/home/MenuCard";
import { ProjectSummary } from "../components/home/ProjectSummary";
import { RecentRecordingsCard } from "../components/home/RecentRecordingsCard";

export function HomePage() {
  return (
    <main className="mx-auto w-full max-w-7xl px-4 py-6 lg:px-6" id="main-content">
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div><p className="m-0 text-xs font-semibold tracking-[0.16em] text-(--interactive)">精准控制台</p><h1 className="mb-0 mt-1 text-2xl font-semibold">工作区</h1><p className="mb-0 mt-2 text-sm text-(--text-muted)">从车辆连接到实时调参，所有操作都保留设备确认状态。</p></div>
        <span className="data-value text-xs text-(--text-muted)">DCTP v1 · RAM / FLASH 独立确认</span>
      </div>
      <section aria-label="应用目的地" className="grid gap-3 md:grid-cols-2">
        <MenuCard actionLabel="进入实时调试" description="实时编辑 RAM 参数、观察编码器和控制环波形，并审阅固化到 Flash 的修改。" icon={WaveSine} title="实时调参与波形" to="/live" />
        <MenuCard actionLabel="打开波形记录" description="管理完整原始遥测批次，导入导出并在独立只读缓冲中回放。" icon={Database} title="数据记录与回放" to="/records" />
        <MenuCard actionLabel="管理参数方案" description="保存、比较并应用可追踪的 RAM 参数方案；应用后仍需单独审阅固化。" icon={BookmarksSimple} title="参数方案" to="/live?panel=snapshots" />
        <MenuCard actionLabel="查看链路诊断" description="检查会话、链路流量、CRC、丢样和前端背压等实时指标。" icon={Pulse} title="连接与链路诊断" to="/diagnostics" />
      </section>
      <section aria-label="车辆与记录摘要" className="mt-4 grid gap-3 lg:grid-cols-[1.4fr_1fr]">
        <ProjectSummary />
        <RecentRecordingsCard />
      </section>
    </main>
  );
}
