// Workspace-level actions: tabs, sidebar, refresh, palette.

import { onlyAvailable } from "@/domain/features";
import { LAYER, tabNumberHandlers, useActions } from "@/lib/actions";
import { activeTab, store } from "@/store";
import { confirmDiscard } from "@/services/confirm";
import { openInNewWindow, reconnect, reconnectAll } from "@/services/connect";
import { closeToHome } from "@/services/workspaces";
import { type Workspace } from "@/types";
import { createDatabaseFlow } from "../schema/flows";
import { historyTab, newQueryTab, newTableTab } from "./tabs";
import { tabByNumber } from "@/store/workspaceModel";

/** Back / Forward in the active tab, asking before dropping its unsaved edits. */
export async function stepActiveTab(ws: Workspace, dir: -1 | 1) {
  const t = activeTab(ws);
  if (!t || !(dir < 0 ? t.back : t.forward)?.length) return;
  const dirty = store.get().pendingEdits[t.id] ?? 0;
  const wording = { doing: "Going back", verb: "go back", kind: "unsaved" };
  if (dir > 0) Object.assign(wording, { doing: "Going forward", verb: "go forward" });
  if (await confirmDiscard(dirty, wording)) store.get().stepTab(ws.connectionId, dir);
}

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
    reconnect: () => void reconnect(id).then((ok) => ok && s.toast("success", "Reconnected.")),
    "reconnect-all": () => void reconnectAll(),
    "next-tab": () => s.cycleTab(id, 1),
    "prev-tab": () => s.cycleTab(id, -1),
    "tab-back": () => void stepActiveTab(ws, -1),
    "tab-forward": () => void stepActiveTab(ws, 1),
    "toggle-sidebar": () => s.toggleSidebar(id),
    refresh: () => s.refresh(id),
    "open-anything": () => s.openPalette("anything", id),
    "open-connection": () => s.openPalette("connection", id),
    "new-window": () => void openInNewWindow(id),
    ...tabNumberHandlers((n) => {
      const t = tabByNumber(ws, n);
      if (t) s.setActiveTab(id, t.id);
    }),
  });
}
