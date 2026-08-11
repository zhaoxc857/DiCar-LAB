import { Database, SlidersHorizontal, WaveSine } from "@phosphor-icons/react";
import { useConnectionStore } from "../../stores/connectionStore";
import { Card } from "../ui/card";

export function ProjectSummary() {
  const snapshot = useConnectionStore((state) => state.snapshot);
  const metrics = [
    { icon: SlidersHorizontal, label: `${snapshot?.parameters.length ?? 0} 个参数`, detail: `${snapshot?.dirtyCount ?? 0} 项未固化` },
    { icon: WaveSine, label: `${snapshot?.telemetryDescriptors.length ?? 0} 个遥测通道`, detail: "最多同时选择 8 路" },
    { icon: Database, label: `存储代 ${snapshot?.storageGeneration ?? 0}`, detail: "RAM / Flash 独立追踪" },
  ];
  return (
    <Card className="p-4">
      <div className="mb-3 flex items-center justify-between"><h2 className="m-0 text-sm font-semibold">当前项目</h2><span className="font-mono text-[11px] text-(--text-muted)">CAR-01 / DEFAULT</span></div>
      <div className="grid gap-2 sm:grid-cols-3">
        {metrics.map(({ icon: MetricIcon, label, detail }) => (
          <div className="flex items-center gap-3 rounded-[var(--radius)] border border-(--border) bg-(--surface) p-3" key={label}>
            <MetricIcon aria-hidden="true" className="shrink-0 text-(--interactive)" size={20} />
            <div><strong className="block text-sm">{label}</strong><span className="text-[11px] text-(--text-muted)">{detail}</span></div>
          </div>
        ))}
      </div>
    </Card>
  );
}
