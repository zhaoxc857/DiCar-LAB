import builtInYaml from "./builtins/dicar-diff-drive.yaml?raw";
import { parseVehicleProfile } from "./parser";
import type { VehicleProfileV1 } from "./types";

export const GENERIC_PROFILE_ID = "generic-manifest";

export type StoredVehicleProfile = {
  source: "builtIn" | "user";
  profile: VehicleProfileV1;
  yamlText: string;
};

export const builtInProfiles: StoredVehicleProfile[] = [storedBuiltIn(builtInYaml)];

function storedBuiltIn(yamlText: string): StoredVehicleProfile {
  return { source: "builtIn", profile: parseVehicleProfile(yamlText), yamlText };
}
