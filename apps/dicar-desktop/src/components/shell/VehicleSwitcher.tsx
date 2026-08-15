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
  return <section aria-labelledby="vehicle-preferences-title" className="app-surface p-4">
    <div className="flex items-center gap-2">
      <CarProfile aria-hidden="true" className="text-(--interactive)" size={20} />
      <h3 className="m-0 text-sm font-semibold" id="vehicle-preferences-title">车型偏好</h3>
    </div>
    <label className="mt-4 block text-xs text-(--text-muted)">
      <span className="mb-1.5 block">车型配置</span>
      <select
        aria-label="车型配置"
        className="data-value h-10 w-full rounded-[var(--radius)] border border-(--border) bg-(--background) px-3 text-xs text-(--text)"
        onChange={(event) => selectProfile(event.target.value)}
        value={selectedProfileId}
      >
        <option value={GENERIC_PROFILE_ID}>通用 Manifest</option>
        {profiles.map(({ source, profile }) => <option key={profile.vehicle.id} value={profile.vehicle.id}>{profile.vehicle.displayName} · {source === "builtIn" ? "内置" : "用户"}</option>)}
      </select>
    </label>
    <button aria-label="管理车型配置" className="mt-3 flex min-h-10 w-full items-center justify-center gap-2 rounded-[var(--radius)] border border-(--border) text-xs font-semibold text-(--text-muted) hover:border-(--interactive) hover:text-(--interactive)" onClick={() => setManagerOpen(true)} type="button"><GearSix aria-hidden="true" size={17} />管理车型配置</button>
    <VehicleProfileManager onClose={() => setManagerOpen(false)} open={managerOpen} />
  </section>;
}
