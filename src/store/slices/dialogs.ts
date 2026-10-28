// Prompt / confirm / alert / host-key dialogs as promises: `await dialog(spec)`.

import { type StateCreator } from "zustand";
import { uid } from "@/lib/uid";
import { type ArtName, type HostKeyError } from "@/types";
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

/** A message with only an OK button (form errors, test results). */
export interface AlertSpec {
  kind: "alert";
  title: string;
  message: string;
  /** Listed under the message: the fields to fill in, warnings… */
  details?: string[];
  hint?: string;
  tone?: "error" | "warning" | "success";
  /** A brand illustration in place of the tone's icon. */
  art?: ArtName;
}

export interface HostKeySpec {
  kind: "hostKey";
  error: HostKeyError;
}

export type DialogSpec = PromptSpec | ConfirmSpec | AlertSpec | HostKeySpec;

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
