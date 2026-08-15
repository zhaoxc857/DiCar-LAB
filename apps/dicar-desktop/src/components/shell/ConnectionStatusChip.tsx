import { LinkBreak, PlugsConnected } from "@phosphor-icons/react";
import { endpointLabel } from "../../domain/types";
import { connectionLabel, useConnectionStore } from "../../stores/connectionStore";

export function ConnectionStatusChip({ onOpen }: { onOpen: () => void }) {
  const snapshot = useConnectionStore((state) => state.snapshot);
  const hydrated = useConnectionStore((state) => state.hydrated);
  const label = hydrated ? connectionLabel(snapshot) : "载入状态";
  const ready = snapshot?.phase === "ready";
  const endpoint = snapshot?.transportIdentity === null || snapshot?.transportIdentity === undefined
    ? "尚未建立"
    : endpointLabel(snapshot.transportIdentity.endpoint);

  return (
    <button
      aria-label={`${label}，打开设备连接`}
      className="flex min-h-11 min-w-0 items-center gap-2 rounded-[var(--radius)] border border-(--border) bg-(--surface) px-3 text-left hover:border-(--interactive) hover:bg-(--surface-hover)"
      onClick={onOpen}
      type="button"
    >
      <span className={ready ? "text-(--success)" : "text-(--warning)"}>
        {ready
          ? <PlugsConnected aria-hidden="true" size={18} weight="fill" />
          : <LinkBreak aria-hidden="true" size={18} />}
      </span>
      <span className="min-w-0">
        <output aria-live="polite" className="block text-xs font-semibold">{label}</output>
        <span className="data-value block max-w-40 truncate text-[10px] text-(--text-muted)">{endpoint}</span>
      </span>
    </button>
  );
}
