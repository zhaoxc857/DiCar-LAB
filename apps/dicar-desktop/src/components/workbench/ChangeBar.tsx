import { useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import { useCollaborationStore } from "../../stores/collaborationStore";
import { Button } from "../ui/button";

type ChangeBarProps = { dirtyCount: number; onReview: () => void };

export function ChangeBar({ dirtyCount, onReview }: ChangeBarProps) {
  if (dirtyCount === 0) return null;
  return <VisibleChangeBar dirtyCount={dirtyCount} onReview={onReview} />;
}

function VisibleChangeBar({ dirtyCount, onReview }: ChangeBarProps) {
  const bridge = useDesktopBridge();
  const profile = useCollaborationStore((state) => state.profile);
  const [message, setMessage] = useState<string | null>(null);
  const commitReason = profile.role !== "owner" ? "当前身份没有固化权限" : !profile.leaseActive ? "当前车辆控制权未激活" : null;
  async function run(action: "undo" | "revert") {
    const result = action === "undo" ? await bridge.undoLast() : await bridge.revertAll();
    setMessage(result.message);
  }
  return <aside className="sticky bottom-3 z-20 mt-4 flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius)] border border-(--border) bg-[color-mix(in_srgb,var(--surface-raised)_94%,transparent)] p-3 shadow-xl backdrop-blur">
    <div><strong className="text-sm">{dirtyCount} 项待固化</strong><p className="m-0 mt-0.5 text-[10px] text-(--text-muted)">{message ?? commitReason ?? "RAM 修改已由设备确认，可审阅后固化"}</p></div>
    <div className="flex gap-2"><Button onClick={() => void run("undo")} size="sm" variant="secondary">撤销上次</Button><Button onClick={() => void run("revert")} size="sm" variant="secondary">全部回退</Button><Button disabled={commitReason !== null} onClick={onReview} size="sm">审阅并固化</Button></div>
  </aside>;
}
