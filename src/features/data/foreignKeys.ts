// Following a foreign key: open the referenced table filtered to the referenced row.

import { cellText } from "@/lib/cells";
import { activeTab, store } from "@/store";
import { type Cell, type Workspace, type WorkspaceTab } from "@/types";
import { tableTab } from "../workspace/tabs";

export function openReferencedRow(
  ws: Workspace,
  from: WorkspaceTab,
  ref: { table: string; column: string },
  value: Cell,
) {
  const id = ws.connectionId;
  const { addTab, updateTab } = store.get();
  addTab(id, tableTab("data", ref.table, from), { reuse: true });
  const opened = activeTab(store.workspace(id) ?? ws);
  if (!opened) return;
  // The data tab applies it once its table state is loaded.
  updateTab(id, opened.id, { pendingFilter: { column: ref.column, value: cellText(value) } });
}
