// Open connections and their tabs. "home" or a connection id is the visible view.

import { type StateCreator } from "zustand";
import { api } from "@/api";
import { defaultSchema } from "@/domain/schema";
import { type ConnectInfo, type NewTab, type Workspace, type WorkspaceTab } from "@/types";
import { persistWorkspaces, savedWorkspace } from "../persistence";
import { type AppState } from "../types";
import * as model from "../workspaceModel";

export const HOME = "home";

export interface WorkspacesSlice {
  workspaces: Record<string, Workspace>;
  /** "home" or a connection id. */
  view: string;
  setView: (view: string) => void;
  openWorkspace: (info: ConnectInfo) => Promise<void>;
  closeWorkspace: (id: string) => Promise<void>;
  updateWorkspace: (id: string, patch: Partial<Workspace>) => void;
  /** Points the sidebar (and new tabs) at another database of the server. */
  switchDatabase: (id: string, database: string) => void;
  toggleSidebar: (id: string) => void;
  refresh: (id: string) => void;
  addTab: (id: string, tab: NewTab, opts?: { reuse?: boolean }) => void;
  updateTab: (id: string, tabId: string, patch: Partial<WorkspaceTab>) => void;
  setActiveTab: (id: string, tabId: string) => void;
  closeTab: (id: string, tabId: string) => void;
  closeOtherTabs: (id: string, tabId: string) => void;
  closeTabsAfter: (id: string, tabId: string) => void;
  closeTabsWhere: (id: string, match: (t: WorkspaceTab) => boolean) => void;
  moveTab: (id: string, tabId: string, toIndex: number) => void;
  cycleTab: (id: string, dir: 1 | -1) => void;
}

export const createWorkspacesSlice: StateCreator<AppState, [], [], WorkspacesSlice> = (
  set,
  get,
) => {
  const save = () => persistWorkspaces(() => get().workspaces);

  /** Applies `fn` to one workspace, if it is open. */
  const patch = (id: string, fn: (ws: Workspace) => Partial<Workspace>) => {
    const ws = get().workspaces[id];
    if (ws) get().updateWorkspace(id, fn(ws));
  };

  return {
    workspaces: {},
    view: HOME,

    setView: (view) => set({ view }),

    openWorkspace: async (info) => {
      const id = info.connectionId;
      const existing = get().workspaces[id];
      if (existing) {
        set({ view: id, workspaces: { ...get().workspaces, [id]: { ...existing, info } } });
        return;
      }
      const [caps, saved] = await Promise.all([
        api.driverCapabilities(info.driver),
        savedWorkspace(id),
      ]);
      set({
        view: id,
        restorable: get().restorable.filter((r) => r !== id),
        workspaces: { ...get().workspaces, [id]: model.restoreWorkspace(info, caps, saved) },
      });
      save();
    },

    closeWorkspace: async (id) => {
      save();
      await api.disconnect(id).catch(() => undefined);
      const { [id]: _closed, ...rest } = get().workspaces;
      const order = Object.keys(get().workspaces);
      set({ workspaces: rest, view: model.viewAfterClose(order, id, get().view) });
      save();
    },

    updateWorkspace: (id, changes) => {
      const ws = get().workspaces[id];
      if (!ws) return;
      set({ workspaces: { ...get().workspaces, [id]: { ...ws, ...changes } } });
      save();
    },

    switchDatabase: (id, database) =>
      patch(id, (ws) => ({ database, schema: defaultSchema(ws.info.driver, database) })),

    toggleSidebar: (id) => patch(id, (ws) => ({ sidebarHidden: !ws.sidebarHidden })),

    refresh: (id) => {
      api.refreshMetadata(id).catch(() => undefined);
      patch(id, (ws) => ({ refreshKey: ws.refreshKey + 1 }));
    },

    addTab: (id, tab, opts) => patch(id, (ws) => model.openTab(ws, tab, !!opts?.reuse)),

    updateTab: (id, tabId, changes) =>
      patch(id, (ws) => ({
        tabs: ws.tabs.map((t) => (t.id === tabId ? { ...t, ...changes } : t)),
      })),

    setActiveTab: (id, tabId) => patch(id, () => ({ activeTabId: tabId })),

    closeTab: (id, tabId) => patch(id, (ws) => model.closeTab(ws, tabId)),

    closeOtherTabs: (id, tabId) =>
      patch(id, (ws) => ({ tabs: ws.tabs.filter((t) => t.id === tabId), activeTabId: tabId })),

    closeTabsAfter: (id, tabId) => patch(id, (ws) => model.closeTabsAfter(ws, tabId)),

    closeTabsWhere: (id, match) => patch(id, (ws) => model.closeTabsWhere(ws, match)),

    moveTab: (id, tabId, toIndex) =>
      patch(id, (ws) => ({ tabs: model.moveTab(ws, tabId, toIndex) })),

    cycleTab: (id, dir) => patch(id, (ws) => ({ activeTabId: model.cycleTab(ws, dir) })),
  };
};
