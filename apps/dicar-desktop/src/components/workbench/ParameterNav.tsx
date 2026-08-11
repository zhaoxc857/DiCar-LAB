import { useMemo, useState } from "react";
import type { ParameterSnapshot } from "../../domain/types";
import { Input } from "../ui/input";

type ParameterNavProps = {
  records: ParameterSnapshot[];
  selectedGroup: string;
  selectedParamId: number | null;
  onSelectGroup: (group: string) => void;
  onSelectParameter: (paramId: number) => void;
};

export function ParameterNav({ records, selectedGroup, selectedParamId, onSelectGroup, onSelectParameter }: ParameterNavProps) {
  const [query, setQuery] = useState("");
  const [modifiedOnly, setModifiedOnly] = useState(false);
  const groups = useMemo(() => [...new Set(records.map(({ group }) => group))], [records]);
  const normalized = query.trim().toLocaleLowerCase();
  const visible = records.filter((record) => (!modifiedOnly || record.dirty) && (!normalized || `${record.displayName} ${record.machineName}`.toLocaleLowerCase().includes(normalized)));

  return <aside className="min-w-0 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3">
    <div className="flex items-center justify-between"><h2 className="m-0 text-sm">参数目录</h2><span className="font-mono text-[10px] text-(--text-muted)">{records.length}</span></div>
    <Input aria-label="搜索参数" className="mt-3 h-9 text-xs" onChange={(event) => setQuery(event.currentTarget.value)} placeholder="搜索名称或 machine_name" value={query} />
    <label className="mt-2 flex items-center gap-2 text-xs text-(--text-muted)"><input checked={modifiedOnly} className="accent-(--interactive)" onChange={(event) => setModifiedOnly(event.currentTarget.checked)} type="checkbox" />仅显示已修改</label>
    <nav aria-label="参数分组" className="mt-4 space-y-1">
      {groups.map((group) => {
        const count = visible.filter((record) => record.group === group).length;
        return <div key={group}>
          <button aria-expanded={selectedGroup === group} className={`flex w-full items-center justify-between rounded px-2.5 py-2 text-left text-xs ${selectedGroup === group ? "bg-[color-mix(in_srgb,var(--interactive)_14%,transparent)] text-(--interactive)" : "text-(--text-muted) hover:bg-(--surface) hover:text-(--text)"}`} onClick={() => onSelectGroup(group)} type="button"><span>{group}</span><span className="font-mono text-[10px]">{count}</span></button>
          {selectedGroup === group && <div className="ml-2 border-l border-(--border) pl-2">{visible.filter((record) => record.group === group).map((record) => <button aria-current={selectedParamId === record.paramId ? "true" : undefined} className={`my-0.5 block w-full rounded px-2 py-1.5 text-left text-[11px] ${selectedParamId === record.paramId ? "bg-(--surface) text-(--text)" : "text-(--text-muted) hover:text-(--text)"}`} key={record.paramId} onClick={() => onSelectParameter(record.paramId)} type="button"><span className="block truncate">{record.displayName}</span><span className="block truncate font-mono text-[9px] opacity-60">{record.machineName}</span></button>)}</div>}
        </div>;
      })}
    </nav>
  </aside>;
}
