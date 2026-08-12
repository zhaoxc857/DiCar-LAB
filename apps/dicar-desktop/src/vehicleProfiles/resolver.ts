import type { ParameterSnapshot, TelemetryDescriptor } from "../domain/types";
import type {
  CompatibilityIssue,
  ResolvedControlLoop,
  ResolvedParameterSection,
  ResolvedScopePreset,
  ResolvedVehicleWorkspace,
  VehicleProfileV1,
} from "./types";

export function resolveVehicleWorkspace(profile: VehicleProfileV1, parameters: readonly ParameterSnapshot[], telemetry: readonly TelemetryDescriptor[]): ResolvedVehicleWorkspace {
  const parameterByName = new Map(parameters.map((record) => [record.machineName, record]));
  const telemetryByName = new Map(telemetry.map((descriptor) => [descriptor.machineName, descriptor]));
  const issues: CompatibilityIssue[] = [];
  const controlLoops = profile.controlLoops.flatMap((loop, index) => {
    const path = `control_loops[${index}]`;
    const target = loop.targetParameter === undefined ? undefined : parameterByName.get(loop.targetParameter);
    if (loop.targetParameter !== undefined && target === undefined) issue(issues, "error", `${path}.target_parameter`, `设备清单缺少参数 ${loop.targetParameter}`);
    const targetNumeric = target !== undefined && target.ramValue.kind !== "bool";
    if (target !== undefined && !targetNumeric) issue(issues, "error", `${path}.target_parameter`, "目标参数必须是数值类型");
    else if (target !== undefined && !target.writable) issue(issues, "warning", `${path}.target_parameter`, "目标参数只读，保留显示但不能写入");

    const gainParamIds = Object.entries(loop.gains).flatMap(([label, name]) => {
      const record = parameterByName.get(name);
      if (record === undefined) { issue(issues, "error", `${path}.gains.${label}`, `设备清单缺少参数 ${name}`); return []; }
      if (record.ramValue.kind === "bool") { issue(issues, "error", `${path}.gains.${label}`, `增益参数 ${name} 不是数值类型`); return []; }
      return [{ label, paramId: record.paramId }];
    });
    const targetChannel = resolveChannel(loop.telemetry.target, `${path}.telemetry.target`, telemetryByName, issues);
    const feedbackChannel = resolveChannel(loop.telemetry.feedback, `${path}.telemetry.feedback`, telemetryByName, issues);
    const errorChannel = resolveChannel(loop.telemetry.error, `${path}.telemetry.error`, telemetryByName, issues);
    const outputs = loop.telemetry.outputs.flatMap((name, outputIndex) => {
      const channel = resolveChannel(name, `${path}.telemetry.outputs[${outputIndex}]`, telemetryByName, issues);
      return channel === null ? [] : [channel];
    });
    const implicit = [targetChannel, feedbackChannel, errorChannel, ...outputs].filter((id): id is number => id !== null);
    const recommended = loop.recommendedChannels.length === 0
      ? implicit
      : loop.recommendedChannels.flatMap((name, channelIndex) => {
        const channel = resolveChannel(name, `${path}.recommended_channels[${channelIndex}]`, telemetryByName, issues);
        return channel === null ? [] : [channel];
      });
    const recommendedChannelIds = unique(recommended);
    const hasContent = (target !== undefined && targetNumeric) || gainParamIds.length > 0 || implicit.length > 0;
    if (!hasContent) { issue(issues, "error", path, "控制环没有任何可用参数或遥测角色"); return []; }
    const resolved: ResolvedControlLoop = {
      id: loop.id,
      label: loop.label,
      category: loop.category,
      hint: loop.hint,
      targetParamId: target !== undefined && targetNumeric ? target.paramId : null,
      targetWritable: target !== undefined && targetNumeric && target.writable,
      gainParamIds,
      telemetry: { target: targetChannel, feedback: feedbackChannel, error: errorChannel, outputs },
      recommendedChannelIds,
    };
    return [resolved];
  });
  const parameterSections = profile.parameterSections.map((section, sectionIndex): ResolvedParameterSection => ({
    id: section.id,
    label: section.label,
    paramIds: section.parameters.flatMap((name, parameterIndex) => {
      const record = parameterByName.get(name);
      if (record === undefined) { issue(issues, "warning", `parameter_sections[${sectionIndex}].parameters[${parameterIndex}]`, `设备清单缺少参数 ${name}`); return []; }
      return [record.paramId];
    }),
  })).filter(({ paramIds }) => paramIds.length > 0);
  const scopePresets = profile.scopePresets.map((preset, presetIndex): ResolvedScopePreset => ({
    id: preset.id,
    label: preset.label,
    channelIds: unique(preset.channels.flatMap((name, channelIndex) => {
      const channel = resolveChannel(name, `scope_presets[${presetIndex}].channels[${channelIndex}]`, telemetryByName, issues);
      return channel === null ? [] : [channel];
    })),
  })).filter(({ channelIds }) => channelIds.length > 0);
  return {
    profileId: profile.vehicle.id,
    displayName: profile.vehicle.displayName,
    type: profile.vehicle.type,
    controlLoops,
    parameterSections,
    scopePresets,
    issues,
    fallbackRequired: controlLoops.length === 0 && parameterSections.length === 0 && scopePresets.length === 0,
  };
}

export function genericVehicleWorkspace(parameters: readonly ParameterSnapshot[], telemetry: readonly TelemetryDescriptor[]): ResolvedVehicleWorkspace {
  const groups = new Map<string, ParameterSnapshot[]>();
  for (const record of parameters) groups.set(record.group, [...(groups.get(record.group) ?? []), record]);
  return {
    profileId: "generic-manifest",
    displayName: "通用 Manifest",
    type: "设备自描述工作区",
    controlLoops: [],
    parameterSections: [...groups.entries()].map(([group, records]) => ({ id: slug(group), label: group, paramIds: records.map(({ paramId }) => paramId) })),
    scopePresets: telemetry.length === 0 ? [] : [{ id: "all", label: "全部通道", channelIds: telemetry.map(({ channelId }) => channelId) }],
    issues: [],
    fallbackRequired: false,
  };
}

function resolveChannel(name: string | undefined, path: string, telemetryByName: ReadonlyMap<string, TelemetryDescriptor>, issues: CompatibilityIssue[]): number | null {
  if (name === undefined) return null;
  const descriptor = telemetryByName.get(name);
  if (descriptor === undefined) { issue(issues, "warning", path, `设备清单缺少遥测 ${name}`); return null; }
  return descriptor.channelId;
}

function issue(issues: CompatibilityIssue[], severity: CompatibilityIssue["severity"], path: string, message: string): void {
  issues.push({ severity, path, message });
}

function unique(values: number[]): number[] { return [...new Set(values)]; }
function slug(value: string): string { return value.toLocaleLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "group"; }
