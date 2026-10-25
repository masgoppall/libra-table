// Opening the tools of a workspace, from menus, the palette and the sidebar.

import { store, tabDb } from "@/store";
import { type Workspace } from "@/types";
import { diagramTab, metricsTab } from "../workspace/tabs";

const where = (ws: Workspace) => ({
  connectionId: ws.connectionId,
  db: tabDb(ws),
  schema: ws.schema,
});

/** `table` null imports into a new table. */
export function openImport(ws: Workspace, table: string | null) {
  store.get().openToolDialog({ kind: "import", table, ...where(ws) });
}

export function openSqlFile(ws: Workspace, database?: string) {
  const w = where(ws);
  const db = database === undefined ? w.db : tabDb(ws, { database });
  store.get().openToolDialog({ kind: "sql-file", ...w, db });
}

export function openExportTables(ws: Workspace, selected: string[] = []) {
  store.get().openToolDialog({ kind: "export-tables", selected, ...where(ws) });
}

export function openDiagram(ws: Workspace) {
  store.get().addTab(ws.connectionId, diagramTab(ws.schema), { reuse: true });
}

export function openMetrics(ws: Workspace) {
  store.get().addTab(ws.connectionId, metricsTab(), { reuse: true });
}
