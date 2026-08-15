import { ArrowSquareOut, Cpu } from "@phosphor-icons/react";
import { Button } from "../ui/button";

export type FirmwareFlashUiState =
  | { kind: "unavailable" }
  | { kind: "checking" }
  | { kind: "selecting" }
  | { kind: "preparing" }
  | { kind: "flashing"; progressPercent: number }
  | { kind: "succeeded" }
  | { kind: "failed"; message: string };

export type FirmwareFlashEntryProps = {
  firmwareVersion: [number, number, number] | null;
  state: FirmwareFlashUiState;
  onOpenFirmwareFlash?: () => void;
};

export function FirmwareFlashEntry({
  firmwareVersion,
  state,
  onOpenFirmwareFlash,
}: FirmwareFlashEntryProps) {
  const version = firmwareVersion === null
    ? "固件版本未知"
    : `固件 ${firmwareVersion.join(".")}`;
  const unavailable = state.kind === "unavailable";
  const stateLabel = firmwareFlashStateLabel(state);

  return (
    <section aria-labelledby="firmware-entry-title" className="app-surface mt-4 p-4">
      <div className="flex items-start gap-3">
        <span className="grid size-9 shrink-0 place-items-center rounded-[var(--radius)] bg-[color-mix(in_srgb,var(--interactive)_10%,transparent)] text-(--interactive)">
          <Cpu aria-hidden="true" size={20} weight="duotone" />
        </span>
        <div className="min-w-0 flex-1">
          <h3 className="m-0 text-sm font-semibold" id="firmware-entry-title">设备固件</h3>
          <p className="data-value m-0 mt-1 text-xs text-(--text-muted)">{version}</p>
        </div>
      </div>
      <p aria-live="polite" className="m-0 mt-3 text-xs text-(--text-muted)">{stateLabel}</p>
      <Button
        className="mt-3 w-full"
        disabled={unavailable}
        onClick={onOpenFirmwareFlash}
        variant="secondary"
      >
        <ArrowSquareOut aria-hidden="true" size={16} />
        {unavailable ? "无线烧录尚未启用" : "打开无线烧录"}
      </Button>
    </section>
  );
}

function firmwareFlashStateLabel(state: FirmwareFlashUiState): string {
  switch (state.kind) {
    case "unavailable": return "无线烧录尚未启用";
    case "checking": return "正在检查设备";
    case "selecting": return "选择固件文件";
    case "preparing": return "正在准备烧录";
    case "flashing": return `烧录中 ${state.progressPercent}%`;
    case "succeeded": return "烧录成功";
    case "failed": return `烧录失败：${state.message}`;
  }
}
