// Connection and group operations from the list's menus and keys (CON-9).

import { api } from "@/api";
import { emptySecrets } from "@/domain/connection";
import { copyText } from "@/lib/clipboard";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type ConnectionView, type Group } from "@/types";

const reload = () => store.get().loadConnections();

/** Runs `fn`, shows its error if any, then reloads the list. */
async function thenReload(fn: () => Promise<unknown>) {
  await fn().catch(showError);
  await reload();
}

export async function deleteConnection(c: ConnectionView) {
  const r = await store.get().dialog({
    kind: "confirm",
    title: "Delete connection",
    message:
      `Delete "${c.name}"? Its saved passwords, keys, saved queries and history ` +
      "are removed too.",
    okLabel: "Delete",
    danger: true,
  });
  if (!r.ok) return;
  try {
    await store.get().closeWorkspace(c.id);
    await api.deleteConnection(c.id);
    await reload();
    toastSuccess(`Deleted "${c.name}".`);
  } catch (err) {
    showError(err);
  }
}

export async function duplicateConnection(c: ConnectionView) {
  const r = await store.get().dialog({
    kind: "confirm",
    title: "Duplicate connection",
    message: `Duplicate "${c.name}". Copy its saved passwords and key too?`,
    okLabel: "Duplicate with secrets",
  });
  // Cancel still duplicates, without the secrets.
  await thenReload(() => api.duplicateConnection(c.id, r.ok));
}

export async function copyConnectionUrl(c: ConnectionView) {
  try {
    await copyText(await api.connectionUrl(c.id));
    toastSuccess("URL copied (without password).");
  } catch (err) {
    showError(err);
  }
}

export function moveToGroup(c: ConnectionView, groupId: string | null) {
  return thenReload(() => api.saveConnection({ ...c, groupId }, emptySecrets()));
}

export function toggleGroup(g: Group) {
  return thenReload(() => api.saveGroup({ ...g, collapsed: !g.collapsed }));
}

export async function createGroup() {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "New group",
    label: "Group name",
    placeholder: "e.g. Clients",
    okLabel: "Create",
  });
  const name = r.ok ? r.value?.trim() : "";
  if (!name) return;
  await thenReload(() => api.saveGroup({ id: "", name, parentId: null, collapsed: false }));
}

export async function renameGroup(g: Group) {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Rename group",
    label: "Group name",
    initial: g.name,
  });
  const name = r.ok ? r.value?.trim() : "";
  if (name) await thenReload(() => api.saveGroup({ ...g, name }));
}

export async function deleteGroup(g: Group) {
  const r = await store.get().dialog({
    kind: "confirm",
    title: "Delete group",
    message: `Delete "${g.name}"? Its connections move to "Ungrouped".`,
    okLabel: "Delete",
    danger: true,
  });
  if (r.ok) await thenReload(() => api.deleteGroup(g.id));
}
