import { HARDWARE_PROFILES } from "../../domain/hardwareProfiles";
import type { SerialHardwareProfile } from "../../domain/types";

export function HardwareConnectionGuide({ profile }: { profile: SerialHardwareProfile }) {
  const definition = HARDWARE_PROFILES[profile];
  return (
    <aside aria-label={`${definition.label} 连接说明`} className="border-b border-(--border) bg-(--background) px-4 py-2 text-[11px] text-(--text-muted) lg:px-6">
      <div className="flex flex-wrap items-center gap-x-5 gap-y-1">
        <strong className="text-(--text)">{definition.label}</strong>
        {definition.guidance.map((item) => <span key={item}>• {item}</span>)}
      </div>
      {definition.warning !== null && <p className="m-0 mt-1 text-(--warning)">{definition.warning}</p>}
    </aside>
  );
}
