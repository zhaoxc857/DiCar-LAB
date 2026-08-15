import type { ReactNode } from "react";
import type { WorkbenchMode } from "../../stores/settingsStore";
import { cn } from "../../lib/cn";

type WorkbenchLayoutProps = {
  mode: WorkbenchMode;
  navigation: ReactNode;
  editor: ReactNode;
  waveform: ReactNode;
};

export function WorkbenchLayout({ mode, navigation, editor, waveform }: WorkbenchLayoutProps) {
  return (
    <div
      className={cn(
        "mt-3 grid min-h-[560px] gap-3",
        mode === "standard"
          ? "xl:grid-cols-[264px_minmax(420px,1fr)_minmax(440px,1.15fr)]"
          : "xl:grid-cols-[196px_minmax(320px,.78fr)_minmax(520px,1.45fr)]",
      )}
      data-testid="workbench-layout"
      data-workbench-mode={mode}
    >
      <div className={cn("min-w-0", mode === "track" ? "space-y-2" : "space-y-3")}>{navigation}</div>
      <div className="min-w-0">{editor}</div>
      <div className="min-w-0">{waveform}</div>
    </div>
  );
}
