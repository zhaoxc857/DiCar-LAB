import { BookmarksSimple, Database, Robot } from "@phosphor-icons/react";
import { Link } from "react-router";
import { Button } from "../ui/button";

type WorkbenchContextActionsProps = {
  onOpenAutoTune: () => void;
  onOpenSnapshots: () => void;
  revision: number;
};

export function WorkbenchContextActions({ onOpenAutoTune, onOpenSnapshots, revision }: WorkbenchContextActionsProps) {
  return (
    <div className="flex flex-wrap items-center justify-end gap-2">
      <Button onClick={onOpenAutoTune} size="sm" type="button" variant="secondary"><Robot aria-hidden="true" size={15} />AI 调参</Button>
      <Button onClick={onOpenSnapshots} size="sm" type="button" variant="secondary"><BookmarksSimple aria-hidden="true" size={15} />参数方案</Button>
      <Link aria-label="打开波形记录库" className="inline-flex min-h-8 items-center justify-center gap-2 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) px-3 text-xs font-semibold text-(--text) no-underline transition-colors hover:border-(--interactive)" to="/records"><Database aria-hidden="true" size={15} />波形记录</Link>
      <span className="data-value text-[10px] text-(--text-muted)">RAM ≠ FLASH · ACK TRUTH · REV {revision}</span>
    </div>
  );
}
