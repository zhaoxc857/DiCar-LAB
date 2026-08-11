import { create } from "zustand";
import type { AccessProfile } from "../domain/types";

type CollaborationState = {
  profile: AccessProfile;
  setProfile: (profile: AccessProfile) => void;
  reset: () => void;
};

const ownerProfile: AccessProfile = { role: "owner", leaseActive: true, localDemoOnly: true };

export const useCollaborationStore = create<CollaborationState>((set) => ({
  profile: ownerProfile,
  setProfile: (profile) => set({ profile }),
  reset: () => set({ profile: ownerProfile }),
}));
