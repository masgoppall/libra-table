// Editing keyboard shortcuts: record a key press for an action, warn about clashes,
// save to Settings (Rust also updates the native menu's accelerators).

import { useState } from "react";
import { type ActionId, conflictsWith, keyLabel, keysOf, SHORTCUTS } from "@/lib/actions";
import { showError } from "@/services/errors";
import { store, useSettings } from "@/store";

function labelOf(id: ActionId): string {
  return SHORTCUTS.find((s) => s.id === id)?.label ?? id;
}

export function useShortcutEditor() {
  const custom = useSettings()?.shortcuts ?? {};
  const [recording, setRecording] = useState<ActionId | null>(null);

  const save = (shortcuts: Record<string, string>) =>
    store.get().saveSettings({ shortcuts }).catch(showError);

  const assign = async (id: ActionId, keys: string) => {
    const clashes = conflictsWith(keys, id);
    if (clashes.length) {
      const r = await store.get().dialog({
        kind: "confirm",
        title: "Shortcut already used",
        message:
          `${keyLabel(keys)} is used by ${clashes.map(labelOf).join(", ")}. ` +
          `Use it for ${labelOf(id)} instead?`,
        okLabel: "Use it",
      });
      if (!r.ok) return;
    }
    await save({ ...custom, [id]: keys });
  };

  /** Key handler for the recording field: the next shortcut-like press is taken. */
  const onRecordKey = (e: React.KeyboardEvent) => {
    if (!recording) return;
    // Keep the press away from the app's own shortcuts and the native menu.
    e.preventDefault();
    e.stopPropagation();
    const keys = keysOf(e.nativeEvent);
    if (!keys) return;
    const id = recording;
    setRecording(null);
    void assign(id, keys);
  };

  const reset = (id: ActionId) => {
    const { [id]: _removed, ...rest } = custom;
    void save(rest);
  };

  return {
    custom,
    recording,
    record: setRecording,
    onRecordKey,
    reset,
    resetAll: () => void save({}),
  };
}
