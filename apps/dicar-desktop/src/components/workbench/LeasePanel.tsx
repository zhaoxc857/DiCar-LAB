import { useState } from "react";
import { useDesktopBridge } from "../../app/providers";
import type { AccessProfileId } from "../../domain/types";
import { useCollaborationStore } from "../../stores/collaborationStore";
import { Select } from "../ui/select";

const roleLabel: Record<AccessProfileId, string> = { owner: "Owner · 调参和固化", tuner: "Tuner · 仅调 RAM", observer: "Observer · 只读" };

export function LeasePanel() {
  const bridge = useDesktopBridge();
  const profile = useCollaborationStore((state) => state.profile);
  const [message, setMessage] = useState<string | null>(null);

  async function select(role: AccessProfileId) {
    const result = await bridge.selectAccessProfile(role);
    setMessage(result.message);
  }

  return <section className="flex flex-wrap items-center gap-3 rounded-[var(--radius)] border border-(--border) bg-(--surface-raised) p-3">
    <div className="mr-auto min-w-56"><h2 className="m-0 text-xs">控制权与协作</h2><p className="m-0 mt-1 text-[10px] text-(--warning)">本地演示权限，不是远程安全边界</p></div>
    <span className={`rounded border px-2 py-1 text-[10px] ${profile.leaseActive ? "border-(--success) text-(--success)" : "border-(--warning) text-(--warning)"}`}>{profile.leaseActive ? "当前主机持有控制权" : "无活动控制权"}</span>
    <div className="min-w-52"><label className="sr-only" htmlFor="access-profile">演示身份</label><Select aria-label="演示身份" className="h-9 text-xs" id="access-profile" onChange={(event) => void select(event.currentTarget.value as AccessProfileId)} value={profile.role}>{Object.entries(roleLabel).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</Select></div>
    <div className="flex gap-3 text-[10px] text-(--text-muted)"><span>观察者 2 人</span><span>排队请求 0</span></div>
    {message && <p aria-live="polite" className="m-0 mt-2 text-[10px] text-(--text-muted)">{message}</p>}
  </section>;
}
