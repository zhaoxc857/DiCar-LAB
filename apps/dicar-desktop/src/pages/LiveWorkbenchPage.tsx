import { WaveSine } from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { AutoTuneWizard } from "../components/workbench/AutoTuneWizard";
import { ChangeBar } from "../components/workbench/ChangeBar";
import { CommitReviewDialog } from "../components/workbench/CommitReviewDialog";
import { ControlLoopWorkspace } from "../components/workbench/ControlLoopWorkspace";
import { EncoderCalibrationPanel } from "../components/workbench/EncoderCalibrationPanel";
import { LeasePanel } from "../components/workbench/LeasePanel";
import { ParameterEditor } from "../components/workbench/ParameterEditor";
import { ParameterNav } from "../components/workbench/ParameterNav";
import { SnapshotManagerDialog } from "../components/workbench/SnapshotManagerDialog";
import { TypedParameterControl } from "../components/workbench/TypedParameterControl";
import { WaveformPanel } from "../components/workbench/WaveformPanel";
import type { WaveformSelectionRequest } from "../components/workbench/WaveformPanel";
import { WorkbenchContextActions } from "../components/workbench/WorkbenchContextActions";
import { WorkbenchLayout } from "../components/workbench/WorkbenchLayout";
import { WorkbenchModeSwitch } from "../components/workbench/WorkbenchModeSwitch";
import { WorkspaceNav, type WorkspaceTask } from "../components/workbench/WorkspaceNav";
import type { ParameterSnapshot } from "../domain/types";
import { useConnectionStore } from "../stores/connectionStore";
import { useSettingsStore } from "../stores/settingsStore";
import { useVehicleProfileStore } from "../stores/vehicleProfileStore";
import { useWorkspaceStore } from "../stores/workspaceStore";
import { namespaceProfileWorkgroups } from "../telemetry/telemetryWorkgroups";
import { builtInProfiles, GENERIC_PROFILE_ID } from "../vehicleProfiles/catalog";
import { genericVehicleWorkspace, resolveVehicleWorkspace } from "../vehicleProfiles/resolver";

export function LiveWorkbenchPage() {
  const snapshot = useConnectionStore((state) => state.snapshot);
  const workbenchMode = useSettingsStore((state) => state.workbenchMode);
  const buffer = useWorkspaceStore((state) => state.buffer);
  const selectedProfileId = useVehicleProfileStore((state) => state.selectedProfileId);
  const userProfiles = useVehicleProfileStore((state) => state.userProfiles);
  const records = useMemo(() => snapshot?.parameters ?? [], [snapshot?.parameters]);
  const telemetry = useMemo(() => snapshot?.telemetryDescriptors ?? [], [snapshot?.telemetryDescriptors]);
  const profile = [...builtInProfiles, ...userProfiles].find((entry) => entry.profile.vehicle.id === selectedProfileId)?.profile;
  const workspace = useMemo(() => {
    if (selectedProfileId === GENERIC_PROFILE_ID || profile === undefined) return genericVehicleWorkspace(records, telemetry);
    const resolved = resolveVehicleWorkspace(profile, records, telemetry);
    if (!resolved.fallbackRequired) return resolved;
    const generic = genericVehicleWorkspace(records, telemetry);
    return { ...generic, displayName: `通用 Manifest · ${profile.vehicle.displayName} 不兼容`, issues: resolved.issues };
  }, [profile, records, selectedProfileId, telemetry]);
  const groups = useMemo(() => [...new Set(records.map(({ group }) => group))], [records]);
  const [selectedTask, setSelectedTask] = useState<WorkspaceTask>({ kind: "all", id: "all" });
  const [selectedGroup, setSelectedGroup] = useState("速度环 PID");
  const [selectedParamId, setSelectedParamId] = useState<number | null>(null);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [snapshotsOpen, setSnapshotsOpen] = useState(false);
  const [autoTuneOpen, setAutoTuneOpen] = useState(false);
  const [waveformRequest, setWaveformRequest] = useState<WaveformSelectionRequest | null>(null);
  const requestId = useRef(0);
  const recommendationSignature = useRef<string | null>(null);

  useEffect(() => {
    const available = availableTasks(workspace);
    if (!available.some((task) => task.kind === selectedTask.kind && task.id === selectedTask.id)) setSelectedTask(available[0] ?? { kind: "all", id: "all" });
  }, [groups, selectedTask.id, selectedTask.kind, workspace]);

  const selectedLoop = selectedTask.kind === "loop" ? workspace.controlLoops.find(({ id }) => id === selectedTask.id) : undefined;
  const selectedLoopSignature = selectedLoop === undefined ? null : `${workspace.profileId}|${selectedLoop.id}|${selectedLoop.recommendedChannelIds.join(",")}|${telemetry.map(({ channelId, machineName, telemetryType }) => `${channelId}:${machineName}:${telemetryType}`).join(";")}`;
  useEffect(() => {
    if (selectedLoop === undefined || selectedLoopSignature === null) { recommendationSignature.current = null; return; }
    if (recommendationSignature.current === null) { recommendationSignature.current = selectedLoopSignature; return; }
    if (recommendationSignature.current !== selectedLoopSignature) {
      recommendationSignature.current = selectedLoopSignature;
      setWaveformRequest({ requestId: ++requestId.current, label: `${selectedLoop.label}推荐`, channelIds: selectedLoop.recommendedChannelIds });
    }
  }, [selectedLoop, selectedLoopSignature]);

  useEffect(() => {
    if (!groups.includes(selectedGroup) && groups[0]) setSelectedGroup(groups[0]);
    if (selectedParamId === null || !records.some(({ paramId }) => paramId === selectedParamId)) setSelectedParamId(records.find(({ group }) => group === selectedGroup)?.paramId ?? records[0]?.paramId ?? null);
  }, [groups, records, selectedGroup, selectedParamId]);

  const selectedRecord = records.find(({ paramId }) => paramId === selectedParamId) ?? records.find(({ group }) => group === selectedGroup) ?? null;
  const dirty = records.filter(({ dirty }) => dirty);
  function chooseGroup(group: string) { setSelectedGroup(group); setSelectedParamId(records.find((record) => record.group === group)?.paramId ?? null); }
  function chooseTask(task: WorkspaceTask) {
    setSelectedTask(task);
    if (task.kind === "loop") {
      const loop = workspace.controlLoops.find(({ id }) => id === task.id);
      if (loop && loop.recommendedChannelIds.length > 0) {
        recommendationSignature.current = `${workspace.profileId}|${loop.id}|${loop.recommendedChannelIds.join(",")}|${telemetry.map(({ channelId, machineName, telemetryType }) => `${channelId}:${machineName}:${telemetryType}`).join(";")}`;
        setWaveformRequest({ requestId: ++requestId.current, label: `${loop.label}推荐`, channelIds: loop.recommendedChannelIds });
      }
    } else if (task.kind === "section") {
      const preset = workspace.scopePresets.find(({ id }) => id === task.id);
      if (preset) setWaveformRequest({ requestId: ++requestId.current, label: preset.label, channelIds: preset.channelIds });
    }
    if (task.kind === "group") chooseGroup(task.id);
    if (task.kind === "all") setSelectedParamId(records[0]?.paramId ?? null);
  }

  return <main className="w-full px-3 py-4 lg:px-5" id="main-content">
    <header className="mb-4 flex flex-wrap items-end justify-between gap-3"><div className="flex items-center gap-3"><WaveSine className="text-(--interactive)" size={28} /><div><h1 className="m-0 text-xl">实时调参与波形</h1><p className="m-0 mt-1 text-xs text-(--text-muted)">{workspace.displayName} · {records.length} 个设备参数 · {telemetry.length} 个遥测通道</p></div></div><div className="flex flex-wrap items-center justify-end gap-2"><WorkbenchModeSwitch /><WorkbenchContextActions onOpenAutoTune={() => setAutoTuneOpen(true)} onOpenSnapshots={() => setSnapshotsOpen(true)} revision={snapshot?.revision ?? 0} /></div></header>
    <LeasePanel />
    <WorkbenchLayout
      editor={<TaskEditor buffer={buffer} records={records} selectedGroup={selectedGroup} selectedRecord={selectedRecord} task={selectedTask} telemetry={telemetry} workspace={workspace} />}
      mode={workbenchMode}
      navigation={<><WorkspaceNav onSelectTask={chooseTask} records={records} selectedTask={selectedTask} workspace={workspace} />{(selectedTask.kind === "all" || selectedTask.kind === "group") && <ParameterNav onSelectGroup={chooseGroup} onSelectParameter={setSelectedParamId} records={selectedTask.kind === "group" ? records.filter((record) => record.group === selectedTask.id) : records} selectedGroup={selectedGroup} selectedParamId={selectedParamId} />}</>}
      waveform={<WaveformPanel descriptors={telemetry} profileWorkgroups={workspace.profileId === GENERIC_PROFILE_ID ? [] : namespaceProfileWorkgroups(workspace.scopePresets)} selectionRequest={waveformRequest} />}
    />
    <ChangeBar dirtyCount={dirty.length} onReview={() => setReviewOpen(true)} />
    <CommitReviewDialog onClose={() => setReviewOpen(false)} open={reviewOpen} records={dirty} />
    <SnapshotManagerDialog onClose={() => setSnapshotsOpen(false)} open={snapshotsOpen} />
    <AutoTuneWizard onClose={() => setAutoTuneOpen(false)} open={autoTuneOpen} records={records} workspace={workspace} />
  </main>;
}

function TaskEditor({ task, workspace, records, selectedGroup, selectedRecord, telemetry, buffer }: { task: WorkspaceTask; workspace: ReturnType<typeof genericVehicleWorkspace>; records: ParameterSnapshot[]; selectedGroup: string; selectedRecord: ParameterSnapshot | null; telemetry: NonNullable<ReturnType<typeof useConnectionStore.getState>["snapshot"]>["telemetryDescriptors"]; buffer: ReturnType<typeof useWorkspaceStore.getState>["buffer"] }) {
  if (task.kind === "loop") {
    const loop = workspace.controlLoops.find(({ id }) => id === task.id);
    return loop ? <ControlLoopWorkspace buffer={buffer} descriptors={telemetry} loop={loop} records={records} /> : null;
  }
  if (task.kind === "section") {
    const section = workspace.parameterSections.find(({ id }) => id === task.id);
    const sectionRecords = section ? section.paramIds.map((id) => records.find(({ paramId }) => paramId === id)).filter((record): record is ParameterSnapshot => record !== undefined) : [];
    const baseline = ["encoder.left.ppr", "encoder.right.ppr", "encoder.quadrature_multiplier"].every((name) => sectionRecords.some((record) => record.machineName === name));
    return <section className="min-w-0"><header className="mb-3"><h2 className="m-0 text-sm">{section?.label ?? "参数任务"}</h2><p className="m-0 mt-1 text-[11px] text-(--text-muted)">车型配置负责组织；字段类型、范围、可写性仍由设备 Manifest 决定。</p></header>{baseline ? <EncoderCalibrationPanel records={sectionRecords} /> : <div className="space-y-3">{sectionRecords.map((record) => <TypedParameterControl key={record.paramId} record={record} />)}</div>}</section>;
  }
  return <ParameterEditor group={selectedGroup} record={selectedRecord} records={records} />;
}

function availableTasks(workspace: ReturnType<typeof genericVehicleWorkspace>): WorkspaceTask[] {
  return [
    ...workspace.controlLoops.map(({ id }) => ({ kind: "loop", id }) as const),
    ...(workspace.profileId === GENERIC_PROFILE_ID ? [] : workspace.parameterSections.map(({ id }) => ({ kind: "section", id }) as const)),
    { kind: "all", id: "all" },
  ];
}
