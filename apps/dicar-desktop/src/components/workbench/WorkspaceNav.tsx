import type { ParameterSnapshot } from "../../domain/types";
import type { ResolvedVehicleWorkspace } from "../../vehicleProfiles/types";

export type WorkspaceTask = { kind: "loop" | "section" | "group" | "all"; id: string };

export function WorkspaceNav({ workspace, records, selectedTask, onSelectTask }: { workspace: ResolvedVehicleWorkspace; records: ParameterSnapshot[]; selectedTask: WorkspaceTask; onSelectTask: (task: WorkspaceTask) => void }) {
  const item = (task: WorkspaceTask, label: string, count: number) => <button aria-current={sameTask(task, selectedTask) ? "page" : undefined} className={`flex w-full items-center justify-between rounded px-2.5 py-2 text-left text-xs ${sameTask(task, selectedTask) ? "bg-[color-mix(in_srgb,var(--interactive)_14%,transparent)] text-(--interactive)" : "text-(--text-muted) hover:bg-(--surface) hover:text-(--text)"}`} key={`${task.kind}:${task.id}`} onClick={() => onSelectTask(task)} type="button"><span>{label}</span><span aria-hidden="true" className="font-mono text-[10px]">{count}</span></button>;
  const configured = workspace.profileId === "generic-manifest" ? null : <>{workspace.controlLoops.map((loop) => item({ kind: "loop", id: loop.id }, loop.label, loop.gainParamIds.length))}{workspace.parameterSections.map((section) => item({ kind: "section", id: section.id }, section.label, section.paramIds.length))}</>;
  return <aside className="min-w-0 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3"><div className="flex items-center justify-between"><div><h2 className="m-0 text-sm">车辆任务</h2><p className="m-0 mt-1 text-[10px] text-(--text-muted)">{workspace.displayName}</p></div><span className="font-mono text-[10px] text-(--text-muted)">{workspace.type}</span></div>{workspace.issues.length > 0 && <p className="m-0 mt-3 rounded border border-(--warning) px-2 py-1.5 text-[10px] text-(--warning)">兼容性：{workspace.issues.length} 条提示，已保留可用内容</p>}<nav aria-label="车辆工作任务" className="mt-3 space-y-1">{configured}{item({ kind: "all", id: "all" }, "全部参数", records.length)}</nav></aside>;
}

export function sameTask(left: WorkspaceTask, right: WorkspaceTask): boolean { return left.kind === right.kind && left.id === right.id; }
