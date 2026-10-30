// One open connection: sidebar, tabs and the tab contents.

import { useState } from "react";
import { isProduction } from "@/domain/connection";
import { tabDb, useConnection } from "@/store";
import { type Workspace } from "@/types";
import { ExportDialog } from "../export/ExportDialog";
import { ToolDialogHost } from "../import/ToolDialogHost";
import { useServerActions } from "../server/serverActions";
import { DatabaseRail } from "../sidebar/DatabaseRail";
import { Sidebar } from "../sidebar/Sidebar";
import { TabBar } from "./TabBar";
import { TabPanes } from "./TabPanes";
import { useToolActions } from "./useToolActions";
import { useWorkspaceActions } from "./useWorkspaceActions";
import { WorkspaceWelcome } from "./WorkspaceWelcome";

interface TableRef {
  schema: string;
  table: string;
}

export function WorkspaceView({ ws, visible }: { ws: Workspace; visible: boolean }) {
  const conn = useConnection(ws.connectionId);
  const [exportTable, setExportTable] = useState<TableRef | null>(null);
  useWorkspaceActions(ws, visible);
  useServerActions(ws, visible);
  useToolActions(ws, visible);
  const classes = `workspace${isProduction(conn) ? " production" : ""}`;
  return (
    <div className={classes} style={{ display: visible ? "flex" : "none" }}>
      <DatabaseRail ws={ws} />
      <Sidebar ws={ws} onExportTable={(schema, table) => setExportTable({ schema, table })} />
      <div className="content">
        {ws.tabs.length > 0 ? <TabBar ws={ws} /> : <WorkspaceWelcome />}
        <TabPanes ws={ws} visible={visible} />
      </div>
      {exportTable && (
        <ExportDialog
          ws={ws}
          db={tabDb(ws)}
          defaultName={exportTable.table}
          tableSource={{
            request: { ...exportTable, page: 0, pageSize: 0, sort: [], filters: [] },
            hasFilters: false,
          }}
          onClose={() => setExportTable(null)}
        />
      )}
      <ToolDialogHost ws={ws} />
    </div>
  );
}
