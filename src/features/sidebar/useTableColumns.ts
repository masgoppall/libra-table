// Columns under a table in the sidebar, loaded the first time it is expanded.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { showError } from "@/services/errors";
import { tabDb } from "@/store";
import { type ColumnInfo, type Workspace } from "@/types";

export type ColumnsState = ColumnInfo[] | "loading";
export type TableColumns = ReturnType<typeof useTableColumns>;

export function useTableColumns(ws: Workspace) {
  const [columns, setColumns] = useState<Record<string, ColumnsState>>({});
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const db = tabDb(ws);

  // A new schema, database or refresh drops what was loaded (and folds the tables).
  useEffect(() => {
    setColumns({});
    setExpanded({});
  }, [ws.connectionId, db, ws.schema, ws.refreshKey]);

  const drop = (name: string) => setColumns(({ [name]: _gone, ...rest }) => rest);

  const toggle = async (name: string) => {
    const open = !expanded[name];
    setExpanded((x) => ({ ...x, [name]: open }));
    if (!open || columns[name]) return;
    setColumns((c) => ({ ...c, [name]: "loading" }));
    try {
      const cols = await api.listColumns(ws.connectionId, db, ws.schema, name);
      setColumns((c) => ({ ...c, [name]: cols }));
    } catch (e) {
      showError(e);
      drop(name);
    }
  };

  return { columns, expanded, toggle };
}
