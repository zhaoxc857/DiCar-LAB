import { isScalar, parseDocument, visit, type Pair } from "yaml";
import {
  VehicleProfileParseError,
  type VehicleControlLoop,
  type VehicleParameterSection,
  type VehicleProfileV1,
  type VehicleScopePreset,
} from "./types";

const MAX_TEXT_LENGTH = 256 * 1024;
const MAX_COLLECTION_ITEMS = 32;
const MAX_REFERENCES = 64;
const idPattern = /^[a-z0-9][a-z0-9_-]*$/;
const topLevelKeys = new Set(["schema_version", "vehicle", "control_loops", "parameter_sections", "scope_presets", "metadata"]);

export function parseVehicleProfile(text: string): VehicleProfileV1 {
  if (text.length > MAX_TEXT_LENGTH) throw new VehicleProfileParseError("", "车型配置不能超过 256 KiB");
  const document = parseDocument(text, { uniqueKeys: true });
  if (document.errors.length > 0) throw new VehicleProfileParseError("YAML", document.errors[0].message);
  validateYamlGraph(document);
  const root = expectObject(document.toJS({ maxAliasCount: 0 }), "配置根节点");
  for (const key of Object.keys(root)) if (!topLevelKeys.has(key)) fail(key, "未知字段");
  if (root.schema_version !== 1) fail("schema_version", "仅支持版本 1");
  const vehicle = parseVehicle(expectObject(root.vehicle, "vehicle"));
  const controlLoops = parseLimited(root.control_loops, "control_loops", parseControlLoop);
  const parameterSections = parseLimited(root.parameter_sections, "parameter_sections", parseParameterSection);
  const scopePresets = parseLimited(root.scope_presets, "scope_presets", parseScopePreset);
  uniqueIds(controlLoops, "control_loops");
  uniqueIds(parameterSections, "parameter_sections");
  uniqueIds(scopePresets, "scope_presets");
  return { schemaVersion: 1, vehicle, controlLoops, parameterSections, scopePresets };
}

function validateYamlGraph(document: ReturnType<typeof parseDocument>): void {
  visit(document, {
    Pair: (_key, pair: Pair) => {
      if (isScalar(pair.key) && pair.key.value === "<<") fail("YAML", "YAML merge key 不受支持");
    },
  });
  visit(document, {
    Alias: () => fail("YAML", "YAML 别名不受支持"),
    Node: (_key, node) => {
      if ("anchor" in node && node.anchor) fail("YAML", "YAML 锚点不受支持");
    },
  });
}

function parseVehicle(value: Record<string, unknown>): VehicleProfileV1["vehicle"] {
  onlyKeys(value, "vehicle", ["id", "display_name", "type", "order"]);
  return {
    id: expectId(value.id, "vehicle.id"),
    displayName: expectString(value.display_name, "vehicle.display_name"),
    type: expectString(value.type, "vehicle.type"),
    order: expectInteger(value.order, "vehicle.order"),
  };
}

function parseControlLoop(raw: unknown, path: string): VehicleControlLoop {
  const value = expectObject(raw, path);
  onlyKeys(value, path, ["id", "label", "category", "hint", "target_parameter", "gains", "telemetry", "recommended_channels"]);
  const gains = stringMap(value.gains, `${path}.gains`);
  const telemetry = expectOptionalObject(value.telemetry, `${path}.telemetry`);
  onlyKeys(telemetry, `${path}.telemetry`, ["target", "feedback", "error", "outputs"]);
  return {
    id: expectId(value.id, `${path}.id`),
    label: expectString(value.label, `${path}.label`),
    category: optionalString(value.category, `${path}.category`),
    hint: optionalString(value.hint, `${path}.hint`),
    targetParameter: optionalString(value.target_parameter, `${path}.target_parameter`),
    gains,
    telemetry: {
      target: optionalString(telemetry.target, `${path}.telemetry.target`),
      feedback: optionalString(telemetry.feedback, `${path}.telemetry.feedback`),
      error: optionalString(telemetry.error, `${path}.telemetry.error`),
      outputs: references(telemetry.outputs, `${path}.telemetry.outputs`),
    },
    recommendedChannels: references(value.recommended_channels, `${path}.recommended_channels`),
  };
}

function parseParameterSection(raw: unknown, path: string): VehicleParameterSection {
  const value = expectObject(raw, path);
  onlyKeys(value, path, ["id", "label", "parameters"]);
  return { id: expectId(value.id, `${path}.id`), label: expectString(value.label, `${path}.label`), parameters: references(value.parameters, `${path}.parameters`) };
}

function parseScopePreset(raw: unknown, path: string): VehicleScopePreset {
  const value = expectObject(raw, path);
  onlyKeys(value, path, ["id", "label", "channels"]);
  return { id: expectId(value.id, `${path}.id`), label: expectString(value.label, `${path}.label`), channels: references(value.channels, `${path}.channels`) };
}

function parseLimited<T>(raw: unknown, path: string, parse: (value: unknown, path: string) => T): T[] {
  if (raw === undefined) return [];
  const values = expectArray(raw, path);
  if (values.length > MAX_COLLECTION_ITEMS) fail(path, `最多 32 个，当前 ${values.length} 个`);
  return values.map((value, index) => parse(value, `${path}[${index}]`));
}

function stringMap(raw: unknown, path: string): Record<string, string> {
  if (raw === undefined) return {};
  const value = expectObject(raw, path);
  if (Object.keys(value).length > MAX_REFERENCES) fail(path, `最多 64 个引用`);
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [expectString(key, `${path}.key`), expectString(item, `${path}.${key}`)]));
}

function references(raw: unknown, path: string): string[] {
  if (raw === undefined) return [];
  const values = expectArray(raw, path);
  if (values.length > MAX_REFERENCES) fail(path, `最多 64 个引用`);
  const result = values.map((value, index) => expectString(value, `${path}[${index}]`));
  const seen = new Set<string>();
  result.forEach((value, index) => {
    if (seen.has(value)) fail(`${path}[${index}]`, `重复引用 ${value}`);
    seen.add(value);
  });
  return result;
}

function uniqueIds(values: Array<{ id: string }>, path: string): void {
  const seen = new Set<string>();
  values.forEach(({ id }, index) => {
    if (seen.has(id)) fail(`${path}[${index}].id`, `重复 ID ${id}`);
    seen.add(id);
  });
}

function expectObject(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail(path, "必须是对象");
  return value as Record<string, unknown>;
}

function expectOptionalObject(value: unknown, path: string): Record<string, unknown> {
  return value === undefined ? {} : expectObject(value, path);
}

function expectArray(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) fail(path, "必须是列表");
  return value;
}

function expectString(value: unknown, path: string): string {
  if (typeof value !== "string" || value.trim() === "") fail(path, "必须是非空字符串");
  return value.trim();
}

function optionalString(value: unknown, path: string): string | undefined {
  return value === undefined ? undefined : expectString(value, path);
}

function expectId(value: unknown, path: string): string {
  const id = expectString(value, path);
  if (!idPattern.test(id)) fail(path, "只能包含小写 ASCII 字母、数字、短横线和下划线");
  return id;
}

function expectInteger(value: unknown, path: string): number {
  if (!Number.isInteger(value)) fail(path, "必须是整数");
  return value as number;
}

function onlyKeys(value: Record<string, unknown>, path: string, keys: string[]): void {
  const allowed = new Set(keys);
  for (const key of Object.keys(value)) if (!allowed.has(key)) fail(`${path}.${key}`, "未知字段");
}

function fail(path: string, message: string): never {
  throw new VehicleProfileParseError(path, message);
}
