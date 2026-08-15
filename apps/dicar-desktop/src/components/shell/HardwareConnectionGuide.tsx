import { HARDWARE_PROFILES } from "../../domain/hardwareProfiles";
import type { SerialHardwareProfile } from "../../domain/types";

export function HardwareConnectionGuide({ profile }: { profile: SerialHardwareProfile }) {
  const definition = HARDWARE_PROFILES[profile];
  return (
    <aside aria-label={`${definition.label} 连接说明`} className="app-surface p-4 text-xs text-(--text-muted)">
      <h3 className="m-0 text-sm font-semibold text-(--text)">{definition.label}</h3>
      <ol className="mb-0 mt-3 space-y-2 pl-5 leading-5">
        {definition.guidance.map((item) => <li key={item}>{item}</li>)}
      </ol>
      {definition.warning !== null && (
        <p className="mb-0 mt-3 rounded-[var(--radius)] border border-(--warning) bg-[color-mix(in_srgb,var(--warning)_8%,transparent)] p-3 text-(--warning)">
          {definition.warning}
        </p>
      )}
    </aside>
  );
}
