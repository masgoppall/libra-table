// Opening a saved connection as a workspace.

import { api } from "@/api";
import { store } from "@/store";
import { type ConnectionView } from "@/types";
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
