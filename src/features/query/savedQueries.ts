// Saved queries (QRY-7): files per connection, opened as query tabs.

import { api } from "@/api";
import { showError } from "@/services/errors";
import { store } from "@/store";
import { newQueryTab } from "../workspace/tabs";

/** Shows the tab of a saved query, opening it if needed. */
export async function openSavedQuery(connectionId: string, name: string) {
  const ws = store.workspace(connectionId);
  const existing = ws?.tabs.find((t) => t.kind === "query" && t.savedName === name);
  if (existing) {
    store.get().setActiveTab(connectionId, existing.id);
    return;
  }
  try {
    const sql = await api.readSavedQuery(connectionId, name);
    store.get().addTab(connectionId, newQueryTab(sql, { title: name, savedName: name }));
  } catch (e) {
    showError(e);
  }
}

/** True when renamed. */
export async function renameSavedQuery(connectionId: string, name: string): Promise<boolean> {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Rename saved query",
    label: "Name",
    initial: name,
  });
  const to = r.ok ? r.value?.trim() : "";
  if (!to || to === name) return false;
  await api.renameSavedQuery(connectionId, name, to).catch(showError);
  return true;
}

/** True when deleted. */
export async function deleteSavedQuery(connectionId: string, name: string): Promise<boolean> {
  const r = await store.get().dialog({
    kind: "confirm",
    title: "Delete saved query",
    message: `Delete "${name}"?`,
    okLabel: "Delete",
    danger: true,
  });
  if (!r.ok) return false;
  await api.deleteSavedQuery(connectionId, name).catch(showError);
  return true;
}
