// The table designer's model: rows for columns, indexes and foreign keys, each with
// what it was (orig) and what it is now (cur). The change they make is in designerChange.

import { blankColumn, columnSpec } from "@/domain/ddl";
import { uid } from "@/lib/uid";
import {
  type ColumnInfo,
  type ColumnSpec,
  type Dialect,
  type ForeignKeyInfo,
  type ForeignKeySpec,
  type IndexInfo,
  type IndexSpec,
  type TableStructure,
} from "@/types";

interface Row<Orig, Cur> {
  key: string;
  /** Absent for rows added in the designer. */
  orig?: Orig;
  cur: Cur;
  dropped: boolean;
}

export interface ColRow extends Row<ColumnSpec, ColumnSpec> {
  /** Position in the original primary key (to keep its order). */
  origPk?: number | null;
  /** Position in the table as loaded (to tell which columns moved). */
  origIndex?: number;
  /** The column as the server described it (its raw EXTRA). */
  info?: ColumnInfo;
}
export type IdxRow = Row<IndexInfo, IndexSpec>;
export type FkRow = Row<ForeignKeyInfo, ForeignKeySpec>;

export interface DesignerState {
  creating: boolean;
  driver: Dialect;
  schema: string;
  /** The existing table's name ("" while creating one). */
  table: string;
  /** The Name field: the new table's name, or a rename of the existing one. */
  name: string;
  comment: string;
  cols: ColRow[];
  idx: IdxRow[];
  fks: FkRow[];
}

export const norm = (s: string | null | undefined) => (s ?? "").trim();
const same = (a: string | null | undefined, b: string | null | undefined) =>
  norm(a).toLowerCase() === norm(b).toLowerCase();

export const live = <T extends { dropped: boolean }>(rows: T[]) => rows.filter((r) => !r.dropped);

// ------------------------------------------------------------------ building rows

export function newColumnRow(driver: Dialect, first: boolean): ColRow {
  return { key: uid(), cur: blankColumn(driver, first), dropped: false };
}

export function newIndexRow(): IdxRow {
  const cur: IndexSpec = { name: "", columns: [], unique: false, algorithm: "" };
  return { key: uid(), cur, dropped: false };
}

export function newForeignKeyRow(column: string): FkRow {
  const cur: ForeignKeySpec = {
    name: "",
    columns: [column],
    refSchema: "",
    refTable: "",
    refColumns: [],
    onUpdate: "NO ACTION",
    onDelete: "NO ACTION",
  };
  return { key: uid(), cur, dropped: false };
}

export function rowsFromStructure(s: TableStructure) {
  const cols: ColRow[] = s.columns.map((c, i) => ({
    key: uid(),
    orig: columnSpec(c),
    origPk: c.pkPosition,
    origIndex: i,
    info: c,
    cur: columnSpec(c),
    dropped: false,
  }));
  const idx: IdxRow[] = s.indexes.map((i) => ({
    key: uid(),
    orig: i,
    cur: { name: i.name, columns: i.columns, unique: i.unique, algorithm: i.algorithm ?? "" },
    dropped: false,
  }));
  const fks: FkRow[] = s.foreignKeys.map((f) => ({
    key: uid(),
    orig: f,
    cur: {
      name: f.name,
      columns: f.columns,
      refSchema: "",
      refTable: f.refTable,
      refColumns: f.refColumns,
      onUpdate: f.onUpdate,
      onDelete: f.onDelete,
    },
    dropped: false,
  }));
  return { cols, idx, fks };
}

// ------------------------------------------------------------------ editing rows

/** Existing rows toggle "dropped"; rows added in the designer are removed. */
export function toggleDrop<T extends Row<unknown, unknown>>(rows: T[], key: string): T[] {
  return rows.flatMap((r) => {
    if (r.key !== key) return [r];
    return r.orig ? [{ ...r, dropped: !r.dropped }] : [];
  });
}

export function patchRow<T extends Row<unknown, C>, C>(rows: T[], key: string, patch: Partial<C>) {
  return rows.map((r) => (r.key === key ? { ...r, cur: { ...r.cur, ...patch } } : r));
}

/** Moves a row to `to` (an index in the list as it is before the move). */
export function moveRow<T extends { key: string }>(rows: T[], key: string, to: number): T[] {
  const from = rows.findIndex((r) => r.key === key);
  if (from < 0) return rows;
  const next = rows.filter((r) => r.key !== key);
  const at = Math.max(0, Math.min(next.length, to > from ? to - 1 : to));
  next.splice(at, 0, rows[from] as T);
  return next;
}

const isSequenceDefault = (d: string | null) => !!d?.startsWith("nextval(");

const TEXT_TYPES = ["char", "varchar", "tinytext", "text", "mediumtext", "longtext", "enum", "set"];

/** MySQL types with a character set and collation. */
export function isTextType(dataType: string): boolean {
  const base = baseType(dataType);
  return TEXT_TYPES.includes(base);
}

/** `varchar(255) unsigned` → `varchar`. */
export function baseType(dataType: string): string {
  return norm(dataType).toLowerCase().split(/[\s(]/)[0] ?? "";
}

/** Applies a column edit, keeping the column consistent. */
export function editColumn(c: ColumnSpec, patch: Partial<ColumnSpec>): ColumnSpec {
  const cur = { ...c, ...patch };
  // Auto increment implies NOT NULL and no default (except a serial's sequence).
  if (patch.autoIncrement) {
    cur.nullable = false;
    if (!isSequenceDefault(cur.default)) cur.default = null;
  }
  if (patch.autoIncrement === false && isSequenceDefault(cur.default)) cur.default = null;
  if (patch.primaryKey) cur.nullable = false;
  // Numbers, dates… have no character set; a collation belongs to one character set.
  if (patch.dataType !== undefined && !isTextType(cur.dataType)) {
    cur.charset = null;
    cur.collation = null;
  }
  if (patch.charset !== undefined && cur.collation && cur.charset) {
    if (!cur.collation.toLowerCase().startsWith(`${cur.charset.toLowerCase()}_`)) {
      cur.collation = null;
    }
  }
  return cur;
}

/** Changed, ignoring the primary key (handled as one op) and unique (create only). */
export function columnChanged(a: ColumnSpec, b: ColumnSpec): boolean {
  return (
    a.name !== b.name ||
    !same(a.dataType, b.dataType) ||
    a.nullable !== b.nullable ||
    norm(a.default) !== norm(b.default) ||
    a.autoIncrement !== b.autoIncrement ||
    norm(a.comment) !== norm(b.comment) ||
    !same(a.charset, b.charset) ||
    !same(a.collation, b.collation) ||
    !same(a.extra, b.extra)
  );
}

/** Which fields of a column differ from how it was loaded (for highlighting cells). */
export function changedFields(r: ColRow): Set<keyof ColumnSpec> {
  const out = new Set<keyof ColumnSpec>();
  const o = r.orig;
  if (!o) return out;
  const c = r.cur;
  if (o.name !== c.name) out.add("name");
  if (!same(o.dataType, c.dataType)) out.add("dataType");
  if (o.nullable !== c.nullable) out.add("nullable");
  if (norm(o.default) !== norm(c.default)) out.add("default");
  if (o.autoIncrement !== c.autoIncrement || !same(o.extra, c.extra)) out.add("extra");
  if (norm(o.comment) !== norm(c.comment)) out.add("comment");
  if (!same(o.charset, c.charset)) out.add("charset");
  if (!same(o.collation, c.collation)) out.add("collation");
  if (o.primaryKey !== c.primaryKey) out.add("primaryKey");
  return out;
}

export type RowStatus = "dropped" | "added" | "changed" | "";

export function columnStatus(r: ColRow): RowStatus {
  if (r.dropped) return "dropped";
  if (!r.orig) return "added";
  const pkChanged = r.orig.primaryKey !== r.cur.primaryKey;
  return columnChanged(r.orig, r.cur) || pkChanged ? "changed" : "";
}

export function indexStatus(r: IdxRow): RowStatus {
  return r.dropped ? "dropped" : r.orig ? "" : "added";
}

/** Column names that can be picked for indexes and foreign keys. */
export function liveColumnNames(cols: ColRow[]): string[] {
  return live(cols).flatMap((r) => (r.cur.name.trim() ? [r.cur.name.trim()] : []));
}

/** The foreign keys a column is part of (not dropped). */
export function columnForeignKeys(fks: FkRow[], column: string): FkRow[] {
  return live(fks).filter((f) => f.cur.columns.includes(column));
}
