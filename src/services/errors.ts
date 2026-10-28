// Turning errors into words for toasts and callouts.

import { store } from "@/store";
import { type AppError, type ArtName, type ErrorLayer } from "@/types";

const LAYER_LABEL: Partial<Record<ErrorLayer, string>> = { ssh: "SSH", database: "Database" };

export function layerLabel(layer: ErrorLayer | undefined): string | undefined {
  return layer ? LAYER_LABEL[layer] : undefined;
}

export function errorText(e: unknown): { message: string; hint?: string } {
  const err = e as Partial<AppError> | undefined;
  const layer = layerLabel(err?.layer);
  const message = err?.message ?? String(e);
  return { message: layer ? `${layer}: ${message}` : message, hint: err?.hint };
}

export function showError(e: unknown) {
  const { message, hint } = errorText(e);
  store.get().toast("error", message, hint);
}

export function toastSuccess(message: string, detail?: string) {
  store.get().toast("success", message, detail);
}

export function toastInfo(message: string, detail?: string) {
  store.get().toast("info", message, detail);
}

/** A popup for an error that needs reading, not a toast (the connection form, tests). */
export async function alertError(title: string, e: unknown, art?: ArtName): Promise<void> {
  const { message, hint } = errorText(e);
  await store.get().dialog({ kind: "alert", tone: "error", title, message, hint, art });
}
