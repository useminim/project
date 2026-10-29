// Essai d'un workflow enregistré, ouvert depuis la liste.

import { create } from "zustand";
import type { Workflow } from "../bindings/Workflow";

interface TrialState {
  workflow: Workflow | null;
  open: (workflow: Workflow) => void;
  close: () => void;
}

export const useTrialStore = create<TrialState>()((set) => ({
  workflow: null,
  open: (workflow) => set({ workflow }),
  close: () => set({ workflow: null }),
}));
