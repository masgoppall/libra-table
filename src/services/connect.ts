// Opening a saved connection as a workspace.

import { api } from "@/api";
import { store } from "@/store";
import { type ConnectionView, type NewTab } from "@/types";
import { showError, toastSuccess } from "./errors";
import { withConnectPrompts } from "./prompts";

/** Connects (asking for what's missing) and shows the workspace. False if cancelled. */
export async function openConnection(conn: ConnectionView): Promise<boolean> {
  const { workspaces, setView, openWorkspace } = store.get();
  if (workspaces[conn.id]) {
    setView(conn.id);
    return true;
  }
  const info = await withConnectPrompts((prompted) => api.connect(conn.id, prompted), {
    title: conn.name,
    offerSavePassphrase: true,
  });
  if (!info) return false;
  await openWorkspace(info);
  return true;
}

/** Drops the connection's sessions and connects again, keeping its tabs (TablePlus'
 * "Reconnect", ⇧⌘R). False if cancelled or it failed (the error is shown). */
export async function reconnect(connectionId: string): Promise<boolean> {
  const conn = store.connection(connectionId);
  if (!conn || !store.get().workspaces[connectionId]) return false;
  await api.disconnect(connectionId, true).catch(() => undefined);
  try {
    const info = await withConnectPrompts((prompted) => api.connect(connectionId, prompted), {
      title: conn.name,
      offerSavePassphrase: true,
    });
    if (!info) return false;
    await store.get().openWorkspace(info);
    store.get().refresh(connectionId);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}

/** Reconnects every open connection, one after another. */
export async function reconnectAll(): Promise<void> {
  for (const id of Object.keys(store.get().workspaces)) await reconnect(id);
  toastSuccess("Reconnected all connections.");
}

/** Opens a connection in a window of its own (TablePlus' "Open in New Window"); the new
 * window connects with the same session. `tab` moves a tab there. */
export async function openInNewWindow(connectionId: string, tab: NewTab | null = null) {
  try {
    await api.openWindow(connectionId, tab);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}
