import { WaveSine } from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import { ChangeBar } from "../components/workbench/ChangeBar";
import { CommitReviewDialog } from "../components/workbench/CommitReviewDialog";
import { LeasePanel } from "../components/workbench/LeasePanel";
import { ParameterEditor } from "../components/workbench/ParameterEditor";
import { ParameterNav } from "../components/workbench/ParameterNav";
import { WaveformPreviewPanel } from "../components/workbench/WaveformPreviewPanel";
import { useConnectionStore } from "../stores/connectionStore";

export function LiveWorkbenchPage() {
  const snapshot = useConnectionStore((state) => state.snapshot);
  const records = useMemo(() => snapshot?.parameters ?? [], [snapshot?.parameters]);
  const groups = useMemo(() => [...new Set(records.map(({ group }) => group))], [records]);
  const [selectedGroup, setSelectedGroup] = useState("速度环 PID");
  const [selectedParamId, setSelectedParamId] = useState<number | null>(null);
  const [reviewOpen, setReviewOpen] = useState(false);

  useEffect(() => {
    if (!groups.includes(selectedGroup) && groups[0]) setSelectedGroup(groups[0]);
    if (selectedParamId === null || !records.some(({ paramId }) => paramId === selectedParamId)) {
      setSelectedParamId(records.find(({ group }) => group === selectedGroup)?.paramId ?? records[0]?.paramId ?? null);
    }
  }, [groups, records, selectedGroup, selectedParamId]);

  const selectedRecord = records.find(({ paramId }) => paramId === selectedParamId) ?? records.find(({ group }) => group === selectedGroup) ?? null;
  const dirty = records.filter(({ dirty }) => dirty);
  function chooseGroup(group: string) { setSelectedGroup(group); setSelectedParamId(records.find((record) => record.group === group)?.paramId ?? null); }

  return <main className="w-full px-3 py-4 lg:px-5" id="main-content">
    <header className="mb-4 flex flex-wrap items-end justify-between gap-3"><div className="flex items-center gap-3"><WaveSine className="text-(--interactive)" size={28} /><div><h1 className="m-0 text-xl">实时调参与波形</h1><p className="m-0 mt-1 text-xs text-(--text-muted)">车辆 CAR-01 · {records.length} 个设备参数 · {snapshot?.telemetryDescriptors.length ?? 0} 个遥测通道</p></div></div><div className="font-mono text-[10px] text-(--text-muted)">RAM ≠ FLASH · ACK TRUTH · REV {snapshot?.revision ?? 0}</div></header>
    <LeasePanel />
    <div className="mt-3 grid min-h-[560px] gap-3 xl:grid-cols-[264px_minmax(420px,1fr)_minmax(440px,1.15fr)]">
      <ParameterNav onSelectGroup={chooseGroup} onSelectParameter={setSelectedParamId} records={records} selectedGroup={selectedGroup} selectedParamId={selectedParamId} />
      <ParameterEditor group={selectedGroup} record={selectedRecord} records={records} />
      <WaveformPreviewPanel descriptors={snapshot?.telemetryDescriptors ?? []} />
    </div>
    <ChangeBar dirtyCount={dirty.length} onReview={() => setReviewOpen(true)} />
    <CommitReviewDialog onClose={() => setReviewOpen(false)} open={reviewOpen} records={dirty} />
  </main>;
}
