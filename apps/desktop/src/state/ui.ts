// État de l'interface uniquement : section affichée et notifications. Les
// données métier (workflows, documents…) viennent de Rust via `api/`.

import type { ParseKeys } from "i18next";
import { create } from "zustand";

export const sections = ["workflows", "waitingZone", "history", "settings"] as const;

export type Section = (typeof sections)[number];

export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  messageKey: ParseKeys;
}

interface UiState {
  section: Section;
  toasts: Toast[];
  navigate: (section: Section) => void;
  showToast: (kind: ToastKind, messageKey: ParseKeys) => void;
  dismissToast: (id: number) => void;
}

let nextToastId = 1;

export const useUiStore = create<UiState>()((set) => ({
  section: "workflows",
  toasts: [],
  navigate: (section) => set({ section }),
  showToast: (kind, messageKey) =>
    set((state) => ({ toasts: [...state.toasts, { id: nextToastId++, kind, messageKey }] })),
  dismissToast: (id) =>
    set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) })),
}));
