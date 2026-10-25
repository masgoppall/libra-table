// Foreign keys and enum values of a table, for FK navigation and dropdown editors.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { type Db, type ForeignKeyInfo } from "@/types";

export interface TableRelations {
  foreignKeys: ForeignKeyInfo[];
  choices: Record<string, string[]>;
}

const NONE: TableRelations = { foreignKeys: [], choices: {} };

export function useTableRelations(
  connectionId: string,
  db: Db,
  schema: string,
  table: string,
  refreshKey: number,
): TableRelations {
  const [relations, setRelations] = useState<TableRelations>(NONE);
  useEffect(() => {
    let cancelled = false;
    // Both are optional extras: the grid works without them.
    Promise.all([
      api.tableStructure(connectionId, db, schema, table).catch(() => null),
      api.columnChoices(connectionId, db, schema, table).catch(() => ({})),
    ]).then(([structure, choices]) => {
      if (cancelled) return;
      setRelations({ foreignKeys: structure?.foreignKeys ?? [], choices });
    });
    return () => {
      cancelled = true;
    };
  }, [connectionId, db, schema, table, refreshKey]);
  return relations;
}

/** The table and column a value of `column` points to, if it is (part of) a foreign key. */
export function referenceOf(
  foreignKeys: ForeignKeyInfo[],
  column: string,
): { table: string; column: string } | null {
  for (const fk of foreignKeys) {
    const at = fk.columns.indexOf(column);
    const target = fk.refColumns[at];
    if (at >= 0 && target) return { table: fk.refTable, column: target };
  }
  return null;
}
