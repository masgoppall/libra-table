// Quick Look for a data tab: binary cells are fetched whole by primary key.

import { useState } from "react";
import { api } from "@/api";
import { isBinary } from "@/lib/cells";
import { type Cell, type CellContent, type Db, type GridValue, type PageResult } from "@/types";
import { QuickLook } from "./QuickLook";

interface Target {
  connectionId: string;
  db: Db;
  schema: string;
  table: string;
}

function local(value: GridValue): CellContent {
  if (value === null || typeof value !== "object") {
    return value === null ? { kind: "null" } : { kind: "text", text: String(value) };
  }
  return { kind: "text", text: isBinary(value) ? `0x${value.$binary}` : "DEFAULT" };
}

export function useQuickLook(t: Target, result: PageResult | null) {
  const [open, setOpen] = useState<{ title: string; load: () => Promise<CellContent> } | null>(
    null,
  );

  const show = (row: number, col: number, value: GridValue) => {
    const column = result?.columns[col]?.name ?? "";
    const loaded = result?.rows[row];
    const pk = (name: string): Cell =>
      loaded?.[result?.columns.findIndex((c) => c.name === name) ?? -1] ?? null;
    const fetchWhole = isBinary(value) && loaded && result?.primaryKey.length;
    const load = fetchWhole
      ? () =>
          api.fetchCell(t.connectionId, t.db, {
            schema: t.schema,
            table: t.table,
            column,
            pk: Object.fromEntries(result.primaryKey.map((k) => [k, pk(k)])),
          })
      : () => Promise.resolve(local(value));
    setOpen({ title: `${t.table}.${column}`, load });
  };

  const node = open && (
    <QuickLook title={open.title} load={open.load} onClose={() => setOpen(null)} />
  );
  return { show, node };
}
