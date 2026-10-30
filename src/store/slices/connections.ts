// Saved connections and groups, which of them were open in the last session, and the
// driver catalog the picker and the connection form are built from.

import { type StateCreator } from "zustand";
import { api } from "@/api";
import { isConnectionWindow } from "@/lib/windowRole";
import { type ConnectionView, type DriverEntry, type Group } from "@/types";
import { savedOpenConnections } from "../persistence";
import { type AppState } from "../types";

export interface ConnectionsSlice {
  connections: ConnectionView[];
  groups: Group[];
  /** Connection ids whose tabs are saved from a previous session (STO-5). */
  restorable: string[];
  drivers: DriverEntry[];
  loadConnections: () => Promise<void>;
  loadDrivers: () => Promise<void>;
}

export const createConnectionsSlice: StateCreator<AppState, [], [], ConnectionsSlice> = (
  set,
  get,
) => ({
  connections: [],
  groups: [],
  restorable: [],
  drivers: [],

  loadDrivers: async () => set({ drivers: await api.listDrivers() }),

  loadConnections: async () => {
    const { connections, groups } = await api.listConnections();
    set({ connections, groups });
    const known = new Set(connections.map((c) => c.id));
    // A connection window shows its own connection only (App opens it).
    const open = isConnectionWindow ? [] : await savedOpenConnections();
    set({ restorable: open.filter((id) => known.has(id) && !get().workspaces[id]) });
  },
});
