// Open connections and their tabs. "home" or a connection id is the visible view.

import { type StateCreator } from "zustand";
import { api } from "@/api";
import { type ConnectInfo, type NewTab, type Workspace, type WorkspaceTab } from "@/types";
import { persistWorkspaces, savedWorkspace } from "../persistence";
import { type AppState } from "../types";
import { isNavigable, navigateTab, stepTab } from "../tabHistory";
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
  /** Takes a database off the rail and closes its tabs. */
  closeDatabase: (id: string, database: string) => void;
  toggleSidebar: (id: string) => void;
  refresh: (id: string) => void;
  /** `reuse` brings up a tab already showing the object; with "open in new tab" off it
   * also replaces the current table tab (unless `newTab`). */
  addTab: (id: string, tab: NewTab, opts?: { reuse?: boolean; newTab?: boolean }) => void;
  /** Back (-1) or Forward (1) in the active tab's history. */
  stepTab: (id: string, dir: -1 | 1) => void;
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

  /** Applies `fn` to one workspace, if it is open. A new active tab brings the sidebar to
   * its database. */
  const patch = (id: string, fn: (ws: Workspace) => Partial<Workspace>) => {
    const ws = get().workspaces[id];
    if (!ws) return;
    const changes = fn(ws);
    const moved = changes.activeTabId !== undefined && changes.activeTabId !== ws.activeTabId;
    const follow = moved ? model.followActiveTab({ ...ws, ...changes }) : {};
    get().updateWorkspace(id, { ...changes, ...follow });
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

    switchDatabase: (id, database) => patch(id, (ws) => model.withDatabase(ws, database)),

    closeDatabase: (id, database) => patch(id, (ws) => model.closeDatabase(ws, database)),

    toggleSidebar: (id) => patch(id, (ws) => ({ sidebarHidden: !ws.sidebarHidden })),

    refresh: (id) => {
      api.refreshMetadata(id).catch(() => undefined);
      patch(id, (ws) => ({ refreshKey: ws.refreshKey + 1 }));
    },

    addTab: (id, tab, opts) =>
      patch(id, (ws) => {
        const { settings, pendingEdits } = get();
        const current = model.activeTab(ws);
        const inPlace =
          opts?.reuse &&
          !opts.newTab &&
          settings?.openInNewTab === false &&
          current &&
          isNavigable(current) &&
          isNavigable({ ...tab, id: "" }) &&
          !pendingEdits[current.id] &&
          !model.findReusable(ws, tab);
        return inPlace ? navigateTab(ws, current.id, tab) : model.openTab(ws, tab, !!opts?.reuse);
      }),

    stepTab: (id, dir) =>
      patch(id, (ws) => (ws.activeTabId ? stepTab(ws, ws.activeTabId, dir) : {})),

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
