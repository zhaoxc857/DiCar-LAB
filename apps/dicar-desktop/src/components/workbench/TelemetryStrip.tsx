import type { AppSnapshot, ParameterSnapshot, TelemetryDescriptor } from "../../domain/types";
import type { TelemetryRingBuffer } from "../../telemetry/ringBuffer";
import type { ResolvedControlLoop } from "../../vehicleProfiles/types";
import { cn } from "../../lib/cn";

export type TelemetryStripItem = {
  id: "target" | "feedback" | "error" | "subscription" | "drop" | "latency";
  label: string;
  value: string;
  unit: string;
  tone: "default" | "success" | "warning";
};

type TelemetryStripProps = {
  buffer: TelemetryRingBuffer;
  descriptors: TelemetryDescriptor[];
  loop: ResolvedControlLoop | undefined;
  records: ParameterSnapshot[];
  snapshot: AppSnapshot | null;
};

export function TelemetryStrip({ buffer, descriptors, loop, records, snapshot }: TelemetryStripProps) {
  const items = telemetryStripItems(buffer, descriptors, loop, records, snapshot);
  return (
    <section aria-label="实时控制指标" className="grid grid-cols-2 gap-px overflow-hidden rounded-[var(--radius)] border border-(--border) bg-(--border) sm:grid-cols-3 xl:grid-cols-6">
      {items.map((item) => (
        <div className="min-w-0 bg-(--surface-raised) px-3 py-2" key={item.id}>
          <span className="block text-[10px] text-(--text-muted)">{item.label}</span>
          <strong className={cn("data-value mt-1 flex items-baseline gap-1 text-sm", item.tone === "success" && "text-(--success)", item.tone === "warning" && "text-(--warning)")}>
            <span>{item.value}</span>
            {item.unit !== "" && <small className="text-[10px] font-normal text-(--text-muted)">{item.unit}</small>}
          </strong>
        </div>
      ))}
    </section>
  );
}

function telemetryStripItems(
  buffer: TelemetryRingBuffer,
  descriptors: TelemetryDescriptor[],
  loop: ResolvedControlLoop | undefined,
  records: ParameterSnapshot[],
  snapshot: AppSnapshot | null,
): TelemetryStripItem[] {
  const target = loop?.telemetry.target === null || loop?.telemetry.target === undefined
    ? parameterValue(loop?.targetParamId, records)
    : channelValue(loop.telemetry.target, descriptors, buffer);
  const feedback = channelValue(loop?.telemetry.feedback, descriptors, buffer);
  const error = channelValue(loop?.telemetry.error, descriptors, buffer);
  const dropped = snapshot === null
    ? null
    : snapshot.diagnostics.sequenceGapSamples + snapshot.diagnostics.deviceDroppedSamples;

  return [
    { id: "target", label: "目标", ...target, tone: "default" },
    { id: "feedback", label: "反馈", ...feedback, tone: feedback.value === "—" ? "default" : "success" },
    { id: "error", label: "误差", ...error, tone: error.value === "—" ? "default" : "warning" },
    { id: "subscription", label: "订阅", value: snapshot?.activeSubscription ? `${snapshot.activeSubscription.sampleRateHz} Hz` : "—", unit: "", tone: "default" },
    { id: "drop", label: "丢样", value: dropped === null ? "—" : String(dropped), unit: "", tone: dropped !== null && dropped > 0 ? "warning" : "default" },
    { id: "latency", label: "往返时延", value: snapshot === null ? "—" : `${snapshot.diagnostics.lastRttMs} ms`, unit: "", tone: "default" },
  ];
}

function channelValue(
  channelId: number | null | undefined,
  descriptors: TelemetryDescriptor[],
  buffer: TelemetryRingBuffer,
): { value: string; unit: string } {
  if (channelId === null || channelId === undefined) return { value: "—", unit: "" };
  const point = buffer.latest(channelId);
  const descriptor = descriptors.find((item) => item.channelId === channelId);
  return point === undefined
    ? { value: "—", unit: descriptor?.unit ?? "" }
    : { value: point.value.value.toFixed(3), unit: descriptor?.unit ?? "" };
}

function parameterValue(
  paramId: number | null | undefined,
  records: ParameterSnapshot[],
): { value: string; unit: string } {
  if (paramId === null || paramId === undefined) return { value: "—", unit: "" };
  const record = records.find((candidate) => candidate.paramId === paramId);
  if (record === undefined || typeof record.ramValue.value !== "number") return { value: "—", unit: record?.unit ?? "" };
  return { value: record.ramValue.value.toFixed(3), unit: record.unit };
}
