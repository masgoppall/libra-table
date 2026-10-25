// Saves open tabs per connection, debounced. Restoring never connects anything.

import { api } from "@/api";
import { type SavedWorkspace, type Workspace } from "@/types";
import { toSaved } from "./workspaceModel";

const SAVE_DELAY_MS = 400;

let timer: ReturnType<typeof setTimeout> | null = null;

export function persistWorkspaces(current: () => Record<string, Workspace>) {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void save(current()), SAVE_DELAY_MS);
}

async function save(workspaces: Record<string, Workspace>) {
  try {
    const file = await api.getWorkspace();
    const connections: Record<string, SavedWorkspace> = { ...(file.connections ?? {}) };
    for (const ws of Object.values(workspaces)) connections[ws.connectionId] = toSaved(ws);
    await api.saveWorkspace({ connections, open: Object.keys(workspaces) });
  } catch {
    // Workspace persistence is best effort.
  }
}

export async function savedWorkspace(connectionId: string): Promise<SavedWorkspace | undefined> {
  try {
    return (await api.getWorkspace()).connections?.[connectionId];
  } catch {
    return undefined;
  }
}

export async function savedOpenConnections(): Promise<string[]> {
  try {
    return (await api.getWorkspace()).open ?? [];
  } catch {
    return [];
  }
}
