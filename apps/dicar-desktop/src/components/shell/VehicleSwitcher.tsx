import { CarProfile } from "@phosphor-icons/react";
import { useSettingsStore } from "../../stores/settingsStore";

export function VehicleSwitcher() {
  const vehicleId = useSettingsStore((state) => state.vehicleId);
  const setVehicleId = useSettingsStore((state) => state.setVehicleId);
  return (
    <label className="flex items-center gap-2 text-xs text-(--text-muted)">
      <CarProfile aria-hidden="true" size={18} />
      <span>车辆</span>
      <select
        aria-label="选择车辆"
        className="h-9 rounded-[var(--radius)] border border-(--border) bg-(--surface) px-2 font-mono text-xs text-(--text)"
        onChange={(event) => setVehicleId(event.target.value)}
        value={vehicleId}
      >
        <option value="car-01">赛车 01 · 单车会话</option>
        <option disabled value="car-02">赛车 02 · 扩展版本</option>
      </select>
    </label>
  );
}
