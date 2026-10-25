// The table designer's model: rows for columns, indexes and foreign keys, each with
// what it was (orig) and what it is now (cur). Pure functions build the schema change.

import { blankColumn, columnSpec } from "@/domain/ddl";
import { uid } from "@/lib/uid";
import {
  type AlterOp,
  type ColumnSpec,
  type Driver,
  type ForeignKeyInfo,
  type ForeignKeySpec,
  type IndexInfo,
  type IndexSpec,
  type SchemaChange,
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
}
export type IdxRow = Row<IndexInfo, IndexSpec>;
export type FkRow = Row<ForeignKeyInfo, ForeignKeySpec>;

export interface DesignerState {
  creating: boolean;
  schema: string;
  /** The existing table's name, or the name typed for a new one. */
  name: string;
  comment: string;
  cols: ColRow[];
  idx: IdxRow[];
  fks: FkRow[];
}

const norm = (s: string | null | undefined) => (s ?? "").trim();

export const live = <T extends { dropped: boolean }>(rows: T[]) => rows.filter((r) => !r.dropped);

// ------------------------------------------------------------------ building rows

export function newColumnRow(driver: Driver, first: boolean): ColRow {
  return { key: uid(), cur: blankColumn(driver, first), dropped: false };
}

export function newIndexRow(): IdxRow {
  return { key: uid(), cur: { name: "", columns: [], unique: false }, dropped: false };
}

export function newForeignKeyRow(): FkRow {
  const cur: ForeignKeySpec = {
    name: "",
    columns: [],
    refSchema: "",
    refTable: "",
    refColumns: [],
    onUpdate: "NO ACTION",
    onDelete: "NO ACTION",
  };
  return { key: uid(), cur, dropped: false };
}

export function rowsFromStructure(s: TableStructure) {
  const cols: ColRow[] = s.columns.map((c) => ({
    key: uid(),
    orig: columnSpec(c),
    origPk: c.pkPosition,
    cur: columnSpec(c),
    dropped: false,
  }));
  const idx: IdxRow[] = s.indexes.map((i) => ({
    key: uid(),
    orig: i,
    cur: { name: i.name, columns: i.columns, unique: i.unique },
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

const isSequenceDefault = (d: string | null) => !!d?.startsWith("nextval(");

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
  return cur;
}

// ------------------------------------------------------------------ the change

/** Changed, ignoring the primary key (handled as one op) and unique (create only). */
export function columnChanged(a: ColumnSpec, b: ColumnSpec): boolean {
  return (
    a.name !== b.name ||
    norm(a.dataType).toLowerCase() !== norm(b.dataType).toLowerCase() ||
    a.nullable !== b.nullable ||
    norm(a.default) !== norm(b.default) ||
    a.autoIncrement !== b.autoIncrement ||
    norm(a.comment) !== norm(b.comment)
  );
}

export type RowStatus = "dropped" | "added" | "changed" | "";

export function columnStatus(r: ColRow): RowStatus {
  if (r.dropped) return "dropped";
  if (!r.orig) return "added";
  const pkChanged = r.orig.primaryKey !== r.cur.primaryKey;
  return columnChanged(r.orig, r.cur) || pkChanged ? "changed" : "";
}

function clean(c: ColumnSpec): ColumnSpec {
  return {
    ...c,
    name: c.name.trim(),
    dataType: c.dataType.trim(),
    default: norm(c.default) || null,
    comment: norm(c.comment) || null,
  };
}

function columnOps(cols: ColRow[]): AlterOp[] {
  return cols.flatMap((r): AlterOp[] => {
    if (r.orig && r.dropped) return [{ op: "drop_column", name: r.orig.name }];
    if (r.orig && columnChanged(r.orig, r.cur)) {
      return [{ op: "modify_column", original: r.orig, column: clean(r.cur) }];
    }
    if (!r.orig && !r.dropped) return [{ op: "add_column", column: clean(r.cur) }];
    return [];
  });
}

/** One op for the whole key, only when its columns or their order changed. */
function primaryKeyOps(cols: ColRow[]): AlterOp[] {
  const before = cols
    .filter((r) => r.origPk != null)
    .sort((a, b) => (a.origPk ?? 0) - (b.origPk ?? 0));
  const after = live(cols).filter((r) => r.cur.primaryKey);
  const same =
    before.length === after.length && before.every((r, i) => r.key === after[i]?.key && !r.dropped);
  return same ? [] : [{ op: "set_primary_key", columns: after.map((r) => r.cur.name.trim()) }];
}

function indexOps(idx: IdxRow[]): AlterOp[] {
  return idx.flatMap((r): AlterOp[] => {
    if (r.orig && r.dropped) return [{ op: "drop_index", name: r.orig.name }];
    if (!r.orig && !r.dropped) return [{ op: "add_index", index: r.cur }];
    return [];
  });
}

function foreignKeyOps(fks: FkRow[]): AlterOp[] {
  return fks.flatMap((r): AlterOp[] => {
    if (r.orig && r.dropped) return [{ op: "drop_foreign_key", name: r.orig.name }];
    if (!r.orig && !r.dropped) return [{ op: "add_foreign_key", foreignKey: r.cur }];
    return [];
  });
}

/** The change to preview and apply; null when an existing table is unchanged. */
export function buildChange(s: DesignerState): SchemaChange | null {
  if (s.creating) {
    return {
      kind: "create_table",
      schema: s.schema,
      name: s.name.trim(),
      columns: live(s.cols).map((r) => clean(r.cur)),
      indexes: live(s.idx).map((r) => r.cur),
      foreignKeys: live(s.fks).map((r) => r.cur),
      comment: norm(s.comment) || null,
    };
  }
  const ops = [
    ...columnOps(s.cols),
    ...primaryKeyOps(s.cols),
    ...indexOps(s.idx),
    ...foreignKeyOps(s.fks),
  ];
  return ops.length ? { kind: "alter_table", schema: s.schema, name: s.name, ops } : null;
}

/** What stops the change from being applied, first problem first. */
export function validate(s: DesignerState): string[] {
  const p: string[] = [];
  const cols = live(s.cols);
  if (s.creating && !s.name.trim()) p.push("Name the table.");
  if (!cols.length) p.push("Add at least one column.");
  if (cols.some((r) => !r.cur.name.trim())) p.push("Every column needs a name.");
  if (cols.some((r) => !r.cur.dataType.trim())) p.push("Every column needs a type.");
  const names = cols.map((r) => r.cur.name.trim().toLowerCase());
  if (new Set(names).size !== names.length) p.push("Two columns have the same name.");
  const added = <T extends Row<unknown, unknown>>(rows: T[]) => live(rows).filter((r) => !r.orig);
  if (added(s.idx).some((r) => r.cur.columns.length === 0)) {
    p.push("Pick the columns of the new index.");
  }
  const badFk = (f: ForeignKeySpec) =>
    !f.refTable || !f.columns.length || f.columns.length !== f.refColumns.length;
  if (added(s.fks).some((r) => badFk(r.cur))) {
    p.push("A new foreign key needs a table and the same number of columns on both sides.");
  }
  return p;
}

/** Column names that can be picked for indexes and foreign keys. */
export function liveColumnNames(cols: ColRow[]): string[] {
  return live(cols).flatMap((r) => (r.cur.name.trim() ? [r.cur.name.trim()] : []));
}
