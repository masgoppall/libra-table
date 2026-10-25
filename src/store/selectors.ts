// Small hooks over the store, so components subscribe only to what they show.

import { type ConnectionView, type Driver, type DriverEntry, type Workspace } from "@/types";
import { useStore } from "./index";

export function useConnection(id: string): ConnectionView | undefined {
  return useStore((s) => s.connections.find((c) => c.id === id));
}

export function useWorkspace(id: string): Workspace | undefined {
  return useStore((s) => s.workspaces[id]);
}

/** True while the tab is the one on screen (its workspace visible and the tab active). */
export function useIsTabVisible(connectionId: string, tabId: string): boolean {
  return useStore(
    (s) => s.view === connectionId && s.workspaces[connectionId]?.activeTabId === tabId,
  );
}

/** The picker entry of a driver (fields, secret label…); undefined until loaded. */
export function useDriverEntry(driver: Driver): DriverEntry | undefined {
  return useStore((s) => s.drivers.find((d) => d.id === driver));
}

export function useSettings() {
  return useStore((s) => s.settings);
}

/** Non-hook access for flows and event handlers. */
export const store = {
  get: () => useStore.getState(),
  connection: (id: string) => useStore.getState().connections.find((c) => c.id === id),
  workspace: (id: string) => useStore.getState().workspaces[id],
};
