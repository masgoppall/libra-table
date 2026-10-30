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

/** "Reports/Monthly" → ["Reports", "Monthly"]; no folder → [null, name]. */
export function splitSavedName(name: string): [string | null, string] {
  const at = name.indexOf("/");
  return at < 0 ? [null, name] : [name.slice(0, at), name.slice(at + 1)];
}

/** Renames the file and the tabs showing it; true when renamed. */
async function moveTo(connectionId: string, from: string, to: string): Promise<boolean> {
  if (!to || to === from) return false;
  try {
    await api.renameSavedQuery(connectionId, from, to);
  } catch (e) {
    showError(e);
    return false;
  }
  const ws = store.workspace(connectionId);
  const tabs = ws?.tabs.map((t) =>
    t.kind === "query" && t.savedName === from ? { ...t, savedName: to, title: to } : t,
  );
  if (tabs) store.get().updateWorkspace(connectionId, { tabs });
  return true;
}

/** True when renamed. */
export async function renameSavedQuery(connectionId: string, name: string): Promise<boolean> {
  const [folder, base] = splitSavedName(name);
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Rename saved query",
    label: "Name",
    initial: base,
  });
  const to = r.ok ? (r.value?.trim() ?? "") : "";
  return moveTo(connectionId, name, to && folder ? `${folder}/${to}` : to);
}

/** Moves a query into a folder (TablePlus' query folders); an empty folder name moves it
 * out. True when moved. */
export async function moveSavedQuery(connectionId: string, name: string): Promise<boolean> {
  const [folder, base] = splitSavedName(name);
  const r = await store.get().dialog({
    kind: "prompt",
    title: `Move ${base} to a folder`,
    label: "Folder (empty: no folder)",
    initial: folder ?? "",
    placeholder: "e.g. Reports",
  });
  if (!r.ok) return false;
  const to = r.value?.trim() ?? "";
  return moveTo(connectionId, name, to ? `${to}/${base}` : base);
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
