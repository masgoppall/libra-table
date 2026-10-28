// Changing one setting of a saved connection from outside its form (the toolbar's lock).

import { api } from "@/api";
import { emptySecrets } from "@/domain/connection";
import { SAFE_MODES } from "@/domain/safeMode";
import { store } from "@/store";
import { type ConnectionView, type SafeMode } from "@/types";
import { showError, toastSuccess } from "./errors";

/** Saves the connection's safe mode; open sessions follow it right away. */
export async function setSafeMode(c: ConnectionView, safeMode: SafeMode) {
  if (c.options.safeMode === safeMode) return;
  try {
    await api.saveConnection({ ...c, options: { ...c.options, safeMode } }, emptySecrets());
    await store.get().loadConnections();
    const label = SAFE_MODES.find((m) => m.id === safeMode)?.label ?? safeMode;
    toastSuccess(`Safe mode: ${label}.`);
  } catch (e) {
    showError(e);
  }
}
