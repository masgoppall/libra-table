// Workspace-level actions: tabs, sidebar, refresh, palette.

import { LAYER, tabNumberHandlers, useActions } from "@/lib/actions";
import { activeTab, store } from "@/store";
import { type Workspace } from "@/types";
import { createDatabaseFlow } from "../schema/flows";
import { historyTab, newQueryTab, newTableTab } from "./tabs";
import { tabByNumber } from "@/store/workspaceModel";

export function useWorkspaceActions(ws: Workspace, visible: boolean) {
  const id = ws.connectionId;
  const s = store.get();
  const current = activeTab(ws);

  useActions(LAYER.workspace, visible, {
    "new-query": () => s.addTab(id, newQueryTab()),
    "new-table": () => s.addTab(id, newTableTab(ws.schema)),
    "new-database": () => createDatabaseFlow(ws),
    "close-tab": () => (current ? s.closeTab(id, current.id) : false),
    disconnect: () => void s.closeWorkspace(id),
    "next-tab": () => s.cycleTab(id, 1),
    "prev-tab": () => s.cycleTab(id, -1),
    history: () => s.addTab(id, historyTab(), { reuse: true }),
    "toggle-sidebar": () => s.toggleSidebar(id),
    refresh: () => s.refresh(id),
    "open-anything": () => s.openPalette("anything", id),
    "switch-database": () => (ws.caps.switchDatabase ? s.openPalette("database", id) : false),
    ...tabNumberHandlers((n) => {
      const t = tabByNumber(ws, n);
      if (t) s.setActiveTab(id, t.id);
    }),
  });
}
