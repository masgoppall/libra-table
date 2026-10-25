// Saved connections and groups, and which of them were open in the last session.

import { type StateCreator } from "zustand";
import { api } from "@/api";
import { type ConnectionView, type Group } from "@/types";
import { savedOpenConnections } from "../persistence";
import { type AppState } from "../types";

export interface ConnectionsSlice {
  connections: ConnectionView[];
  groups: Group[];
  /** Connection ids whose tabs are saved from a previous session (STO-5). */
  restorable: string[];
  loadConnections: () => Promise<void>;
}

export const createConnectionsSlice: StateCreator<AppState, [], [], ConnectionsSlice> = (
  set,
  get,
) => ({
  connections: [],
  groups: [],
  restorable: [],

  loadConnections: async () => {
    const { connections, groups } = await api.listConnections();
    set({ connections, groups });
    const known = new Set(connections.map((c) => c.id));
    const open = await savedOpenConnections();
    set({ restorable: open.filter((id) => known.has(id) && !get().workspaces[id]) });
  },
});
