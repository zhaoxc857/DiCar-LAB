import { create } from "zustand";
import { builtInProfiles, GENERIC_PROFILE_ID, type StoredVehicleProfile } from "../vehicleProfiles/catalog";
import { parseVehicleProfile } from "../vehicleProfiles/parser";

export const VEHICLE_PROFILE_STORAGE_KEY = "dicar-vehicle-profiles";
const MAX_USER_PROFILES = 16;
const MAX_STORED_BYTES = 2 * 1024 * 1024;

export type ImportProfileResult =
  | { status: "imported"; message: string; profileId: string }
  | { status: "needsReplace"; message: string; profileId: string }
  | { status: "failed"; message: string };

type VehicleProfileState = {
  selectedProfileId: string;
  userProfiles: StoredVehicleProfile[];
  catalogIssues: string[];
  importProfile: (yamlText: string, replaceExisting: boolean) => ImportProfileResult;
  removeUserProfile: (id: string) => void;
  selectProfile: (id: string) => void;
  reset: () => void;
};

type PersistedProfiles = { selectedProfileId: string; userYamlTexts: string[] };

const initial = readPersistedProfiles();

export const useVehicleProfileStore = create<VehicleProfileState>((set, get) => ({
  ...initial,
  importProfile: (yamlText, replaceExisting) => {
    let profile: StoredVehicleProfile;
    try {
      profile = { source: "user", profile: parseVehicleProfile(yamlText), yamlText };
    } catch (error) {
      return { status: "failed", message: error instanceof Error ? `导入失败：${error.message}` : "导入失败：车型配置无效" };
    }
    const id = profile.profile.vehicle.id;
    if (builtInProfiles.some((entry) => entry.profile.vehicle.id === id)) return { status: "failed", message: `导入失败：${id} 是内置车型 ID，不能覆盖` };
    const current = get().userProfiles;
    const existingIndex = current.findIndex((entry) => entry.profile.vehicle.id === id);
    if (existingIndex >= 0 && !replaceExisting) return { status: "needsReplace", message: `车型 ${profile.profile.vehicle.displayName} 已存在，需要确认替换`, profileId: id };
    if (existingIndex < 0 && current.length >= MAX_USER_PROFILES) return { status: "failed", message: `最多保存 ${MAX_USER_PROFILES} 个用户车型配置` };
    const next = existingIndex < 0 ? [...current, profile] : current.map((entry, index) => index === existingIndex ? profile : entry);
    if (storedBytes(next) > MAX_STORED_BYTES) return { status: "failed", message: "用户车型配置总量不能超过 2 MiB" };
    set({ userProfiles: sortProfiles(next) });
    persist(get());
    return { status: "imported", message: `已导入 ${profile.profile.vehicle.displayName}`, profileId: id };
  },
  removeUserProfile: (id) => {
    const next = get().userProfiles.filter((entry) => entry.profile.vehicle.id !== id);
    const selectedProfileId = get().selectedProfileId === id ? GENERIC_PROFILE_ID : get().selectedProfileId;
    set({ userProfiles: next, selectedProfileId });
    persist(get());
  },
  selectProfile: (id) => {
    if (!profileExists(id, get().userProfiles)) return;
    set({ selectedProfileId: id });
    persist(get());
  },
  reset: () => {
    set({ selectedProfileId: GENERIC_PROFILE_ID, userProfiles: [], catalogIssues: [] });
    persist(get());
  },
}));

function readPersistedProfiles(): Pick<VehicleProfileState, "selectedProfileId" | "userProfiles" | "catalogIssues"> {
  const fallback = { selectedProfileId: GENERIC_PROFILE_ID, userProfiles: [], catalogIssues: [] };
  if (typeof localStorage === "undefined") return fallback;
  const raw = localStorage.getItem(VEHICLE_PROFILE_STORAGE_KEY);
  if (raw === null) return fallback;
  try {
    const parsed = JSON.parse(raw) as Partial<PersistedProfiles>;
    const yamlTexts = Array.isArray(parsed.userYamlTexts) ? parsed.userYamlTexts.filter((value): value is string => typeof value === "string").slice(0, MAX_USER_PROFILES) : [];
    const userProfiles: StoredVehicleProfile[] = [];
    const catalogIssues: string[] = [];
    for (const yamlText of yamlTexts) {
      try {
        const profile = parseVehicleProfile(yamlText);
        if (builtInProfiles.some((entry) => entry.profile.vehicle.id === profile.vehicle.id) || userProfiles.some((entry) => entry.profile.vehicle.id === profile.vehicle.id)) {
          catalogIssues.push(`已忽略重复车型 ${profile.vehicle.id}`);
          continue;
        }
        userProfiles.push({ source: "user", profile, yamlText });
      } catch (error) {
        catalogIssues.push(error instanceof Error ? `已忽略无效车型：${error.message}` : "已忽略无效车型");
      }
    }
    const bounded = storedBytes(userProfiles) <= MAX_STORED_BYTES ? sortProfiles(userProfiles) : [];
    if (bounded.length === 0 && userProfiles.length > 0) catalogIssues.push("已忽略超出 2 MiB 限制的用户车型配置");
    const candidate = typeof parsed.selectedProfileId === "string" ? parsed.selectedProfileId : GENERIC_PROFILE_ID;
    return { selectedProfileId: profileExists(candidate, bounded) ? candidate : GENERIC_PROFILE_ID, userProfiles: bounded, catalogIssues };
  } catch {
    return { ...fallback, catalogIssues: ["车型配置缓存损坏，已恢复为通用 Manifest"] };
  }
}

function profileExists(id: string, users: StoredVehicleProfile[]): boolean {
  return id === GENERIC_PROFILE_ID || builtInProfiles.some((entry) => entry.profile.vehicle.id === id) || users.some((entry) => entry.profile.vehicle.id === id);
}

function persist(state: Pick<VehicleProfileState, "selectedProfileId" | "userProfiles">): void {
  if (typeof localStorage === "undefined") return;
  const value: PersistedProfiles = { selectedProfileId: state.selectedProfileId, userYamlTexts: state.userProfiles.map(({ yamlText }) => yamlText) };
  localStorage.setItem(VEHICLE_PROFILE_STORAGE_KEY, JSON.stringify(value));
}

function storedBytes(profiles: StoredVehicleProfile[]): number {
  return profiles.reduce((total, { yamlText }) => total + new TextEncoder().encode(yamlText).byteLength, 0);
}

function sortProfiles(profiles: StoredVehicleProfile[]): StoredVehicleProfile[] {
  return [...profiles].sort((left, right) => left.profile.vehicle.order - right.profile.vehicle.order || left.profile.vehicle.displayName.localeCompare(right.profile.vehicle.displayName, "zh-CN"));
}
