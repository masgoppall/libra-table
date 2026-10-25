// Prompt / confirm / host-key dialogs as promises: `await dialog(spec)`.

import { type StateCreator } from "zustand";
import { uid } from "@/lib/uid";
import { type HostKeyError } from "@/types";
import { type AppState } from "../types";

export interface PromptSpec {
  kind: "prompt";
  title: string;
  message?: string;
  label: string;
  secret?: boolean;
  initial?: string;
  placeholder?: string;
  checkbox?: string;
  checked?: boolean;
  okLabel?: string;
  danger?: boolean;
}

export interface ConfirmSpec {
  kind: "confirm";
  title: string;
  message: string;
  details?: string[];
  okLabel?: string;
  danger?: boolean;
  /** The user must type this to enable the button (dropping a database…). */
  typeToConfirm?: string;
  checkbox?: string;
}

export interface HostKeySpec {
  kind: "hostKey";
  error: HostKeyError;
}

export type DialogSpec = PromptSpec | ConfirmSpec | HostKeySpec;

export type DialogResult = { ok: false } | { ok: true; value?: string; checked?: boolean };

export interface OpenDialog {
  id: string;
  spec: DialogSpec;
  resolve: (r: DialogResult) => void;
}

export interface DialogsSlice {
  dialogs: OpenDialog[];
  dialog: (spec: DialogSpec) => Promise<DialogResult>;
  closeDialog: (id: string, result: DialogResult) => void;
}

export const createDialogsSlice: StateCreator<AppState, [], [], DialogsSlice> = (set, get) => ({
  dialogs: [],

  dialog: (spec) =>
    new Promise<DialogResult>((resolve) => {
      set({ dialogs: [...get().dialogs, { id: uid(), spec, resolve }] });
    }),

  closeDialog: (id, result) => {
    const d = get().dialogs.find((x) => x.id === id);
    set({ dialogs: get().dialogs.filter((x) => x.id !== id) });
    d?.resolve(result);
  },
});
