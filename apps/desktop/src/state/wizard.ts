// Session du wizard de création ou de modification d'un workflow. Elle vit
// dans zustand pour survivre à un changement de section : l'utilisateur
// retrouve son brouillon en revenant sur « Workflows ».

import { create } from "zustand";
import type { WorkflowDraft } from "../bindings/WorkflowDraft";
import type { WorkflowId } from "../bindings/WorkflowId";

export const wizardSteps = ["documents", "actions", "summary"] as const;

export type WizardStep = (typeof wizardSteps)[number];

export interface WizardSession {
  /** Création, ou modification du workflow `workflowId` lu à `version`. */
  mode: "create" | "edit";
  workflowId?: WorkflowId;
  version?: number;
  draft: WorkflowDraft;
  step: WizardStep;
  /** Le brouillon a été modifié depuis l'ouverture. */
  dirty: boolean;
  /** Le nom a été saisi par l'utilisateur (il n'est plus proposé automatiquement). */
  nameEdited: boolean;
}

interface WizardState {
  session: WizardSession | null;
  start: (session: Omit<WizardSession, "step" | "dirty">) => void;
  update: (change: (draft: WorkflowDraft) => WorkflowDraft) => void;
  editName: (name: string) => void;
  goTo: (step: WizardStep) => void;
  close: () => void;
}

export const useWizardStore = create<WizardState>()((set) => ({
  session: null,
  start: (session) => set({ session: { ...session, step: "documents", dirty: false } }),
  update: (change) =>
    set(({ session }) =>
      session ? { session: { ...session, draft: change(session.draft), dirty: true } } : {},
    ),
  editName: (name) =>
    set(({ session }) =>
      session
        ? {
            session: {
              ...session,
              draft: { ...session.draft, name },
              dirty: true,
              nameEdited: true,
            },
          }
        : {},
    ),
  goTo: (step) => set(({ session }) => (session ? { session: { ...session, step } } : {})),
  close: () => set({ session: null }),
}));
