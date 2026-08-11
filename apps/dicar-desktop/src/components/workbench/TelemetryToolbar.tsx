import type { TelemetryDescriptor } from "../../domain/types";
import { useState } from "react";
import { Button } from "../ui/button";
import { Select } from "../ui/select";

type TelemetryToolbarProps = {
  descriptors: TelemetryDescriptor[];
  selectedIds: number[];
  sampleRateHz: number;
  windowSeconds: number;
  paused: boolean;
  error: string | null;
  linkReason: string | null;
  maxChannels: number;
  maxSampleRateHz: number;
  onToggleChannel: (channelId: number) => void;
  onSampleRate: (sampleRateHz: number) => void;
  onWindow: (seconds: number) => void;
  onApply: () => void;
  onPause: () => void;
  onMarker: () => void;
};

const windows = [1, 5, 10, 30, 60];

export function TelemetryToolbar(props: TelemetryToolbarProps) {
  const [channelsOpen, setChannelsOpen] = useState(false);
  const rates = [10, 25, 50, 100, 200, 500].filter((rate) => rate <= props.maxSampleRateHz);
  return <div className="mt-3 rounded-[var(--radius)] border border-(--border) bg-(--background) p-3">
    <div className="flex flex-wrap items-end gap-2">
      <label className="min-w-28 text-[10px] text-(--text-muted)">采样率<Select aria-label="遥测采样率" className="mt-1 h-8 text-xs" onChange={(event) => props.onSampleRate(Number(event.currentTarget.value))} value={props.sampleRateHz}>{rates.map((rate) => <option key={rate} value={rate}>{rate} Hz</option>)}</Select></label>
      <Button onClick={props.onApply} size="sm">应用 {props.sampleRateHz} Hz 订阅</Button>
      <Button onClick={props.onPause} size="sm" variant="secondary">{props.paused ? "继续波形" : "暂停波形"}</Button>
      <Button onClick={props.onMarker} size="sm" variant="secondary">添加标记 M</Button>
      <Button aria-expanded={channelsOpen} onClick={() => setChannelsOpen((open) => !open)} size="sm" variant="secondary">选择通道 {props.selectedIds.length}/{props.maxChannels}</Button>
    </div>
    <div aria-label="时间窗口" className="mt-2 flex flex-wrap gap-1">{windows.map((seconds) => <button aria-pressed={props.windowSeconds === seconds} className={`rounded border px-2 py-1 text-[10px] ${props.windowSeconds === seconds ? "border-(--interactive) text-(--interactive)" : "border-(--border) text-(--text-muted)"}`} key={seconds} onClick={() => props.onWindow(seconds)} type="button">{seconds} 秒</button>)}</div>
    {props.linkReason && <p className="m-0 mt-2 text-[10px] text-(--text-muted)">{props.linkReason}</p>}
    {channelsOpen && <fieldset className="mt-3 grid max-h-32 gap-1 overflow-auto border-0 p-0 sm:grid-cols-2"><legend className="mb-1 text-[10px] text-(--text-muted)">通道候选 {props.selectedIds.length}/{props.maxChannels}</legend>{props.descriptors.map((descriptor) => <label className="flex min-w-0 items-center gap-2 rounded px-1.5 py-1 text-[10px] text-(--text-muted) hover:bg-(--surface)" key={descriptor.channelId}><input aria-label={descriptor.displayName} checked={props.selectedIds.includes(descriptor.channelId)} className="accent-(--interactive)" onChange={() => props.onToggleChannel(descriptor.channelId)} type="checkbox" /><span className="truncate">{descriptor.displayName}</span><span className="ml-auto font-mono opacity-60">{descriptor.unit}</span></label>)}</fieldset>}
    {props.error && <p aria-live="assertive" className="m-0 mt-2 text-[10px] text-(--danger)">{props.error}</p>}
  </div>;
}
