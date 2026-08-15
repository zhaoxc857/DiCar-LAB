import { Cpu, Database, SlidersHorizontal, WaveSine } from "@phosphor-icons/react";
import { useConnectionStore } from "../../stores/connectionStore";
import { useVehicleProfileStore } from "../../stores/vehicleProfileStore";
import { builtInProfiles, GENERIC_PROFILE_ID, type StoredVehicleProfile } from "../../vehicleProfiles/catalog";
import { Card } from "../ui/card";

export function ProjectSummary() {
  const snapshot = useConnectionStore((state) => state.snapshot);
  const selectedProfileId = useVehicleProfileStore((state) => state.selectedProfileId);
  const userProfiles = useVehicleProfileStore((state) => state.userProfiles);
  const vehicleLabel = selectedVehicleLabel(selectedProfileId, userProfiles);
  const firmware = snapshot?.firmwareVersion === null || snapshot?.firmwareVersion === undefined
    ? "固件未知"
    : `固件 ${snapshot.firmwareVersion.join(".")}`;
  const metrics = [
    { icon: SlidersHorizontal, label: `${snapshot?.parameters.length ?? 0} 个参数`, detail: `${snapshot?.dirtyCount ?? 0} 项未固化` },
    { icon: WaveSine, label: `${snapshot?.telemetryDescriptors.length ?? 0} 个遥测通道`, detail: "最多同时选择 8 路" },
    { icon: Database, label: `存储代 ${snapshot?.storageGeneration ?? 0}`, detail: "RAM / Flash 独立追踪" },
  ];
  return (
    <Card className="h-full p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="m-0 text-sm font-semibold">当前车辆</h2>
          <p className="m-0 mt-1 text-xs text-(--text-muted)">车型配置与设备实况</p>
        </div>
        <strong className="text-sm text-(--interactive)">{vehicleLabel}</strong>
      </div>
      <div className="my-3 grid gap-2 sm:grid-cols-2">
        <div className="rounded-[var(--radius)] border border-(--border-subtle) bg-(--surface) p-3">
          <span className="block text-[10px] text-(--text-muted)">设备 ID</span>
          <strong className="data-value mt-1 block text-xs">{snapshot?.deviceIdHex ?? "设备未连接"}</strong>
        </div>
        <div className="rounded-[var(--radius)] border border-(--border-subtle) bg-(--surface) p-3">
          <span className="block text-[10px] text-(--text-muted)">固件</span>
          <strong className="data-value mt-1 flex items-center gap-1.5 text-xs"><Cpu aria-hidden="true" size={14} />{firmware}</strong>
        </div>
      </div>
      <div className="grid gap-2 sm:grid-cols-3">
        {metrics.map(({ icon: MetricIcon, label, detail }) => (
          <div className="flex items-center gap-3 rounded-[var(--radius)] border border-(--border) bg-(--surface) p-3" key={label}>
            <MetricIcon aria-hidden="true" className="shrink-0 text-(--interactive)" size={20} />
            <div><strong className="block text-sm">{label}</strong><span className="text-[11px] text-(--text-muted)">{detail}</span></div>
          </div>
        ))}
      </div>
    </Card>
  );
}

function selectedVehicleLabel(
  selectedProfileId: string,
  userProfiles: StoredVehicleProfile[],
): string {
  if (selectedProfileId === GENERIC_PROFILE_ID) return "通用 Manifest";
  return [...builtInProfiles, ...userProfiles]
    .find((entry) => entry.profile.vehicle.id === selectedProfileId)
    ?.profile.vehicle.displayName ?? "通用 Manifest";
}
