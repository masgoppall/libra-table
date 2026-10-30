// Uncommitted edits of a page: updated cells, new rows, rows marked for deletion.
// Pure functions; the view keeps them in state with an undo stack.

import { DEFAULT, sameValue } from "@/lib/cells";
import { type Cell, type CellInput, type Change, type GridValue, type PageResult } from "@/types";

export interface Pending {
  /** Row index → column name → new value. */
  updates: Map<number, Record<string, CellInput>>;
  inserts: Record<string, CellInput>[];
  deletes: Set<number>;
}

export const emptyPending = (): Pending => ({
  updates: new Map(),
  inserts: [],
  deletes: new Set(),
});

const clone = (p: Pending): Pending => ({
  updates: new Map(p.updates),
  inserts: [...p.inserts],
  deletes: new Set(p.deletes),
});

export function pendingCount(p: Pending): number {
  return p.updates.size + p.inserts.length + p.deletes.size;
}

/** The value a cell shows: the edit if any, else what the page loaded. */
export function cellAt(p: Pending, page: PageResult, row: number, col: number): GridValue {
  const name = page.columns[col]?.name ?? "";
  const base = page.rows.length;
  if (row >= base) {
    const v = p.inserts[row - base]?.[name];
    return v === undefined ? DEFAULT : v;
  }
  const u = p.updates.get(row);
  if (u && name in u) return u[name] ?? null;
  return page.rows[row]?.[col] ?? null;
}

export function isEdited(p: Pending, page: PageResult, row: number, col: number): boolean {
  const name = page.columns[col]?.name;
  return row < page.rows.length && !!name && name in (p.updates.get(row) ?? {});
}

/** Sets a cell; setting it back to the loaded value drops the edit. */
export function editCell(
  p: Pending,
  page: PageResult,
  row: number,
  col: number,
  value: CellInput,
): Pending {
  const name = page.columns[col]?.name;
  if (!name) return p;
  const next = clone(p);
  const base = page.rows.length;
  if (row >= base) {
    next.inserts[row - base] = { ...next.inserts[row - base], [name]: value };
    return next;
  }
  const original: Cell = page.rows[row]?.[col] ?? null;
  const u = { ...(next.updates.get(row) ?? {}) };
  if (sameValue(original, value)) delete u[name];
  else u[name] = value;
  if (Object.keys(u).length) next.updates.set(row, u);
  else next.updates.delete(row);
  return next;
}

export function addInsert(p: Pending): Pending {
  return { ...clone(p), inserts: [...p.inserts, {}] };
}

/** New rows with values (duplicated or pasted rows). */
export function addInserts(p: Pending, rows: Record<string, CellInput>[]): Pending {
  return { ...clone(p), inserts: [...p.inserts, ...rows.map((r) => ({ ...r }))] };
}

/** New rows are removed; loaded rows toggle their "deleted" mark. */
export function toggleDeletes(p: Pending, rows: Set<number>, base: number): Pending {
  const next = clone(p);
  const newRows = [...rows].filter((r) => r >= base).map((r) => r - base);
  for (const i of newRows.sort((a, b) => b - a)) next.inserts.splice(i, 1);
  for (const r of rows) {
    if (r >= base) continue;
    if (next.deletes.has(r)) next.deletes.delete(r);
    else next.deletes.add(r);
  }
  return next;
}

/** The edits as changes for Rust, keyed by primary key. */
export function buildChanges(p: Pending, page: PageResult): Change[] {
  const pkIndex = page.primaryKey.map((k) => [k, page.columns.findIndex((c) => c.name === k)]);
  const pkOf = (r: number) =>
    Object.fromEntries(pkIndex.map(([k, i]) => [k, page.rows[r]?.[i as number] ?? null]));
  const changes: Change[] = [];
  for (const [r, values] of p.updates) {
    if (!p.deletes.has(r)) changes.push({ kind: "update", pk: pkOf(r), values });
  }
  for (const values of p.inserts) changes.push({ kind: "insert", values });
  for (const r of p.deletes) changes.push({ kind: "delete", pk: pkOf(r) });
  return changes;
}

/** How the grid marks a row: added, marked for deletion, or neither. */
export function rowStatus(p: Pending, baseRows: number, r: number) {
  return r >= baseRows ? "inserted" : p.deletes.has(r) ? "deleted" : undefined;
}
