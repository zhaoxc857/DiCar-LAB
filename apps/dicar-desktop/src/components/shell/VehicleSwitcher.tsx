import { CarProfile, GearSix } from "@phosphor-icons/react";
import { useState } from "react";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import { builtInProfiles, GENERIC_PROFILE_ID } from "../../vehicleProfiles/catalog";
import { VehicleProfileManager } from "../vehicleProfiles/VehicleProfileManager";

export function VehicleSwitcher() {
  const [managerOpen, setManagerOpen] = useState(false);
  const selectedProfileId = useVehicleProfileStore((state) => state.selectedProfileId);
  const userProfiles = useVehicleProfileStore((state) => state.userProfiles);
  const selectProfile = useVehicleProfileStore((state) => state.selectProfile);
  const profiles = [...builtInProfiles, ...userProfiles].sort((left, right) => left.profile.vehicle.order - right.profile.vehicle.order || left.profile.vehicle.displayName.localeCompare(right.profile.vehicle.displayName, "zh-CN"));
  return <>
    <label className="flex items-center gap-2 text-xs text-(--text-muted)">
      <CarProfile aria-hidden="true" size={18} />
      <span>车型</span>
      <select
        aria-label="车型配置"
        className="h-9 rounded-[var(--radius)] border border-(--border) bg-(--surface) px-2 font-mono text-xs text-(--text)"
        onChange={(event) => selectProfile(event.target.value)}
        value={selectedProfileId}
      >
        <option value={GENERIC_PROFILE_ID}>通用 Manifest</option>
        {profiles.map(({ source, profile }) => <option key={profile.vehicle.id} value={profile.vehicle.id}>{profile.vehicle.displayName} · {source === "builtIn" ? "内置" : "用户"}</option>)}
      </select>
    </label>
    <button aria-label="管理车型配置" className="grid size-9 place-items-center rounded-[var(--radius)] border border-(--border) text-(--text-muted) hover:border-(--interactive) hover:text-(--interactive)" onClick={() => setManagerOpen(true)} type="button"><GearSix size={17} /></button>
    <VehicleProfileManager onClose={() => setManagerOpen(false)} open={managerOpen} />
  </>;
}
