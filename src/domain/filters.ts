// Filter rows (FLT-*): special column ids and the default row.

import { type ColumnInfo, type DefaultFilterColumn, type FilterRow } from "@/types";

export const ANY_COLUMN = "__any__";
export const RAW_SQL = "__raw__";

export function newFilterRow(column: string, operator: string): FilterRow {
  return { enabled: true, column: column || ANY_COLUMN, operator, value: "", value2: "" };
}

/** The column new filters start on, following the user's setting. */
export function defaultFilterColumn(
  pref: DefaultFilterColumn,
  columns: Pick<ColumnInfo, "name">[],
  primaryKey: string[],
): string {
  if (pref === "any") return ANY_COLUMN;
  if (pref === "pk" && primaryKey[0]) return primaryKey[0];
  return columns[0]?.name ?? ANY_COLUMN;
}
