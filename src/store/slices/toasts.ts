// Short-lived notices in the corner. Errors stay longer.

import { type StateCreator } from "zustand";
import { uid } from "@/lib/uid";
import { type AppState } from "../types";

export type ToastKind = "info" | "success" | "error";

export interface Toast {
  id: string;
  kind: ToastKind;
  message: string;
  detail?: string;
}

const MAX_TOASTS = 4;
const TOAST_MS: Record<ToastKind, number> = { info: 3500, success: 3500, error: 9000 };

export interface ToastsSlice {
  toasts: Toast[];
  toast: (kind: ToastKind, message: string, detail?: string) => void;
  dismissToast: (id: string) => void;
}

export const createToastsSlice: StateCreator<AppState, [], [], ToastsSlice> = (set, get) => ({
  toasts: [],

  toast: (kind, message, detail) => {
    const id = uid();
    set({ toasts: [...get().toasts, { id, kind, message, detail }].slice(-MAX_TOASTS) });
    setTimeout(() => get().dismissToast(id), TOAST_MS[kind]);
  },

  dismissToast: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
});
