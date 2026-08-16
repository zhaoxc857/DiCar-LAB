import { Gauge, SquaresFour } from "@phosphor-icons/react";
import { useSettingsStore } from "../../stores/settingsStore";
import { Button } from "../ui/button";

export function WorkbenchModeSwitch() {
  const mode = useSettingsStore((state) => state.workbenchMode);
  const saveWorkbenchMode = useSettingsStore((state) => state.saveWorkbenchMode);

  return (
    <div
      aria-label="工作台模式"
      className="inline-flex rounded-[var(--radius)] border border-(--border) bg-(--surface) p-0.5"
      role="group"
    >
      <Button
        aria-pressed={mode === "standard"}
        className={mode === "standard" ? "border-(--interactive) bg-[color-mix(in_srgb,var(--interactive)_12%,var(--surface-raised))] text-(--interactive)" : "text-(--text-muted)"}
        onClick={() => saveWorkbenchMode("standard")}
        size="sm"
        type="button"
        variant="ghost"
      >
        <SquaresFour aria-hidden="true" size={14} />标准模式
      </Button>
      <Button
        aria-pressed={mode === "track"}
        className={mode === "track" ? "border-(--interactive) bg-[color-mix(in_srgb,var(--interactive)_12%,var(--surface-raised))] text-(--interactive)" : "text-(--text-muted)"}
        onClick={() => saveWorkbenchMode("track")}
        size="sm"
        type="button"
        variant="ghost"
      >
        <Gauge aria-hidden="true" size={14} />赛道模式
      </Button>
    </div>
  );
}
