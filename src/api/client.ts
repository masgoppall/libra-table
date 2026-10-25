// The one place that talks to Rust. Every failure becomes an AppError.

import { invoke } from "@tauri-apps/api/core";
import { type AppError } from "@/types";

export function isAppError(e: unknown): e is AppError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

/** Normalizes anything thrown by invoke into an AppError. */
export function toAppError(e: unknown): AppError {
  if (isAppError(e)) return e;
  const message = e instanceof Error ? e.message : String(e);
  return { kind: "internal", layer: "app", message };
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw toAppError(e);
  }
}
