import type { TelemetryDescriptor } from "../domain/types";

export type TelemetryWorkgroup = {
  id: string;
  label: string;
  channelIds: number[];
};

const semanticGroups: Array<{ id: string; label: string; pattern: RegExp }> = [
  { id: "speed", label: "速度", pattern: /speed|rpm|velocity|速度|转速/i },
  { id: "heading", label: "航向与姿态", pattern: /yaw|heading|gyro|angular|航向|角速度|陀螺/i },
  { id: "encoder", label: "编码器", pattern: /encoder|pulse|count|编码器|脉冲|计数/i },
  { id: "motor", label: "电机", pattern: /motor|pwm|current|torque|电机|电流|扭矩/i },
  { id: "power", label: "电源", pattern: /battery|voltage|power|adc|电池|电压|电源/i },
];

export function buildTelemetryWorkgroups(descriptors: readonly TelemetryDescriptor[]): TelemetryWorkgroup[] {
  if (descriptors.length === 0) return [];
  const groups = semanticGroups.flatMap(({ id, label, pattern }) => {
    const channelIds = descriptors
      .filter((descriptor) => pattern.test(searchText(descriptor)))
      .map(({ channelId }) => channelId);
    return channelIds.length === 0 ? [] : [{ id, label, channelIds }];
  });
  groups.push({ id: "all", label: "全部通道", channelIds: descriptors.map(({ channelId }) => channelId) });
  return groups;
}

export function clipWorkgroup(group: TelemetryWorkgroup, maxChannels: number): { channelIds: number[]; omittedCount: number } {
  const limit = Math.max(0, Math.floor(maxChannels));
  return {
    channelIds: group.channelIds.slice(0, limit),
    omittedCount: Math.max(0, group.channelIds.length - limit),
  };
}

export function namespaceProfileWorkgroups(groups: readonly TelemetryWorkgroup[]): TelemetryWorkgroup[] {
  return groups.map((group) => ({ ...group, id: `profile:${group.id}` }));
}

export function mergeTelemetryWorkgroups(profileGroups: readonly TelemetryWorkgroup[], automaticGroups: readonly TelemetryWorkgroup[], knownChannelIds: readonly number[]): TelemetryWorkgroup[] {
  const known = new Set(knownChannelIds);
  const ids = new Set<string>();
  return [...profileGroups, ...automaticGroups].flatMap((group) => {
    if (ids.has(group.id)) throw new Error(`重复工作组 ID ${group.id}`);
    ids.add(group.id);
    const channelIds = [...new Set(group.channelIds.filter((channelId) => known.has(channelId)))];
    return channelIds.length === 0 ? [] : [{ ...group, channelIds }];
  });
}

function searchText(descriptor: TelemetryDescriptor): string {
  return `${descriptor.machineName} ${descriptor.displayName} ${descriptor.unit}`;
}
