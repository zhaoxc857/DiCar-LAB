export type VehicleProfileIdentity = {
  id: string;
  displayName: string;
  type: string;
  order: number;
};

export type VehicleLoopTelemetry = {
  target?: string;
  feedback?: string;
  error?: string;
  outputs: string[];
};

export type VehicleControlLoop = {
  id: string;
  label: string;
  category?: string;
  hint?: string;
  targetParameter?: string;
  gains: Record<string, string>;
  telemetry: VehicleLoopTelemetry;
  recommendedChannels: string[];
};

export type VehicleParameterSection = {
  id: string;
  label: string;
  parameters: string[];
};

export type VehicleScopePreset = {
  id: string;
  label: string;
  channels: string[];
};

export type VehicleProfileV1 = {
  schemaVersion: 1;
  vehicle: VehicleProfileIdentity;
  controlLoops: VehicleControlLoop[];
  parameterSections: VehicleParameterSection[];
  scopePresets: VehicleScopePreset[];
};

export class VehicleProfileParseError extends Error {
  constructor(readonly path: string, message: string) {
    super(path ? `${path}：${message}` : message);
    this.name = "VehicleProfileParseError";
  }
}

export type CompatibilityIssue = {
  severity: "error" | "warning" | "info";
  path: string;
  message: string;
};

export type ResolvedControlLoop = {
  id: string;
  label: string;
  category?: string;
  hint?: string;
  targetParamId: number | null;
  targetWritable: boolean;
  gainParamIds: Array<{ label: string; paramId: number }>;
  telemetry: { target: number | null; feedback: number | null; error: number | null; outputs: number[] };
  recommendedChannelIds: number[];
};

export type ResolvedParameterSection = { id: string; label: string; paramIds: number[] };
export type ResolvedScopePreset = { id: string; label: string; channelIds: number[] };
export type ResolvedVehicleWorkspace = {
  profileId: string;
  displayName: string;
  type: string;
  controlLoops: ResolvedControlLoop[];
  parameterSections: ResolvedParameterSection[];
  scopePresets: ResolvedScopePreset[];
  issues: CompatibilityIssue[];
  fallbackRequired: boolean;
};
