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

/** A new group at the top level, or inside `parentId` (the group dialog saves it). */
export function createGroup(parentId: string | null = null) {
  store.get().setGroupEditor({ group: null, parentId });
}

/** Saves a group from the group dialog; true when saved. */
export async function saveGroup(g: Group): Promise<boolean> {
  try {
    await api.saveGroup(g);
    await store.get().loadConnections();
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}

/** Moves a group into another one (null = the top level). */
export function moveGroup(g: Group, parentId: string | null) {
  return thenReload(() => api.saveGroup({ ...g, parentId }));
}

/** Name and icon of a group, in the group dialog. */
export function editGroup(g: Group) {
  store.get().setGroupEditor({ group: g, parentId: g.parentId });
}

export async function deleteGroup(g: Group) {
  const r = await store.get().dialog({
    kind: "confirm",
    title: "Delete group",
    message: `Delete "${g.name}"? Its connections and groups move up one level.`,
    okLabel: "Delete",
    danger: true,
  });
  if (r.ok) await thenReload(() => api.deleteGroup(g.id));
}
