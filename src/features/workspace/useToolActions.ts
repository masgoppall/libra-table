// Workspace actions for imports, exports of several tables, the ER diagram and metrics.

import { LAYER, useActions } from "@/lib/actions";
import { activeTab } from "@/store";
import { type Workspace } from "@/types";
import * as tools from "../import/openTools";

export function useToolActions(ws: Workspace, visible: boolean) {
  const current = activeTab(ws);
  // Import into the table on screen, or into a new one.
  const table = current?.kind === "data" || current?.kind === "structure" ? current.table : null;
  useActions(LAYER.workspace, visible, {
    "import-data": () => tools.openImport(ws, table ?? null),
    "import-new-table": () => tools.openImport(ws, null),
    "import-sql": () => tools.openSqlFile(ws),
    "export-tables": () => tools.openExportTables(ws, table ? [table] : []),
    "open-diagram": () => tools.openDiagram(ws),
    "open-metrics": () => tools.openMetrics(ws),
  });
}
