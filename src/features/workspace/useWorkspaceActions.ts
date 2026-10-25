// Workspace-level actions: tabs, sidebar, refresh, palette.

import { onlyAvailable } from "@/domain/features";
import { LAYER, tabNumberHandlers, useActions } from "@/lib/actions";
import { activeTab, store } from "@/store";
import { closeToHome } from "@/services/workspaces";
import { type Workspace } from "@/types";
import { createDatabaseFlow } from "../schema/flows";
import { historyTab, newQueryTab, newTableTab } from "./tabs";
import { tabByNumber } from "@/store/workspaceModel";

export function useWorkspaceActions(ws: Workspace, visible: boolean) {
  const id = ws.connectionId;
  const s = store.get();
  const current = activeTab(ws);

  const gated = onlyAvailable(ws.caps, {
    "new-query": () => s.addTab(id, newQueryTab()),
    "new-table": () => s.addTab(id, newTableTab(ws.schema)),
    "new-database": () => createDatabaseFlow(ws),
    history: () => s.addTab(id, historyTab(), { reuse: true }),
    "switch-database": () => s.openPalette("database", id),
  });

  useActions(LAYER.workspace, visible, {
    ...gated,
    // ⌘W with no tab left closes the connection, like closing its window.
    "close-tab": () => (current ? s.closeTab(id, current.id) : void closeToHome(id)),
    disconnect: () => void s.closeWorkspace(id),
    "next-tab": () => s.cycleTab(id, 1),
    "prev-tab": () => s.cycleTab(id, -1),
    "toggle-sidebar": () => s.toggleSidebar(id),
    refresh: () => s.refresh(id),
    "open-anything": () => s.openPalette("anything", id),
    ...tabNumberHandlers((n) => {
      const t = tabByNumber(ws, n);
      if (t) s.setActiveTab(id, t.id);
    }),
  });
}
