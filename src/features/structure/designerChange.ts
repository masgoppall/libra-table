// The schema change the designer's rows make: CREATE TABLE for a new table, ALTER TABLE
// ops (plus a rename) for an existing one, and what stops it from being applied.

import { type AlterOp, type ColumnPosition, type ColumnSpec, type SchemaChange } from "@/types";
import {
  type ColRow,
  columnChanged,
  type DesignerState,
  type FkRow,
  type IdxRow,
  live,
  norm,
} from "./designerModel";

function clean(c: ColumnSpec): ColumnSpec {
  return {
    ...c,
    name: c.name.trim(),
    dataType: c.dataType.trim(),
    default: norm(c.default) || null,
    comment: norm(c.comment) || null,
    charset: norm(c.charset) || null,
    collation: norm(c.collation) || null,
    extra: norm(c.extra) || null,
  };
}

const nameOf = (r: ColRow) => r.cur.name.trim();

/** The longest run of rows still in their original order: those need not move. */
function unmoved(rows: ColRow[]): Set<string> {
  const v = rows.map((r) => r.origIndex ?? 0);
  const len = v.map(() => 1);
  const back = v.map(() => -1);
  v.forEach((x, i) => {
    for (let j = 0; j < i; j++) {
      if ((v[j] ?? 0) < x && (len[j] ?? 0) + 1 > (len[i] ?? 0)) {
        len[i] = (len[j] ?? 0) + 1;
        back[i] = j;
      }
    }
  });
  const keys = new Set<string>();
  for (let i = len.indexOf(Math.max(0, ...len)); i >= 0; i = back[i] ?? -1) {
    keys.add(rows[i]?.key ?? "");
  }
  return keys;
}

/**
 * MySQL positions for the columns that moved. The ops run in the rows' order (each
 * renames before the next refers to it), existing columns before added ones, so each
 * existing column outside the longest unmoved run goes after the one before it, then
 * each added column does too, unless only added columns follow it.
 */
export function columnPositions(cols: ColRow[]): Map<string, ColumnPosition> {
  const out = new Map<string, ColumnPosition>();
  const kept = live(cols).filter((r) => r.orig);
  const stay = unmoved(kept);
  kept.forEach((r, i) => {
    if (stay.has(r.key)) return;
    const prev = kept[i - 1];
    out.set(r.key, prev ? { after: nameOf(prev) } : "first");
  });
  const all = live(cols);
  all.forEach((r, i) => {
    if (r.orig || !all.slice(i + 1).some((x) => x.orig)) return;
    const prev = all[i - 1];
    out.set(r.key, prev ? { after: nameOf(prev) } : "first");
  });
  return out;
}

function columnOps(s: DesignerState): AlterOp[] {
  const moves = s.driver === "mysql" ? columnPositions(s.cols) : new Map();
  return s.cols.flatMap((r): AlterOp[] => {
    const position = moves.get(r.key);
    const at = position ? { position } : {};
    if (r.orig && r.dropped) return [{ op: "drop_column", name: r.orig.name }];
    if (r.orig && (columnChanged(r.orig, r.cur) || position)) {
      return [{ op: "modify_column", original: r.orig, column: clean(r.cur), ...at }];
    }
    if (!r.orig && !r.dropped) return [{ op: "add_column", column: clean(r.cur), ...at }];
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
  return same ? [] : [{ op: "set_primary_key", columns: after.map(nameOf) }];
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

/** The Name field when it renames the existing table. */
export function renameTo(s: DesignerState): string | null {
  const to = s.name.trim();
  return !s.creating && to && to !== s.table ? to : null;
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
    ...columnOps(s),
    ...primaryKeyOps(s.cols),
    ...indexOps(s.idx),
    ...foreignKeyOps(s.fks),
  ];
  const rename = renameTo(s);
  if (!ops.length && !rename) return null;
  return { kind: "alter_table", schema: s.schema, name: s.table, ops, renameTo: rename };
}

/** How many changes are waiting (ops plus a rename). */
export function pendingCount(change: SchemaChange | null): number {
  if (change?.kind !== "alter_table") return 0;
  return change.ops.length + (change.renameTo ? 1 : 0);
}

/** What stops the change from being applied, first problem first. */
export function validate(s: DesignerState): string[] {
  const p: string[] = [];
  const cols = live(s.cols);
  if (!s.name.trim()) p.push(s.creating ? "Name the table." : "The table needs a name.");
  if (!cols.length) p.push("Add at least one column.");
  if (cols.some((r) => !r.cur.name.trim())) p.push("Every column needs a name.");
  if (cols.some((r) => !r.cur.dataType.trim())) p.push("Every column needs a type.");
  const names = cols.map((r) => r.cur.name.trim().toLowerCase());
  if (new Set(names).size !== names.length) p.push("Two columns have the same name.");
  const added = <T extends { orig?: unknown; dropped: boolean }>(rows: T[]) =>
    live(rows).filter((r) => !r.orig);
  if (added(s.idx).some((r) => r.cur.columns.length === 0)) {
    p.push("Pick the columns of the new index.");
  }
  const badFk = (r: FkRow) =>
    !r.cur.refTable || !r.cur.columns.length || r.cur.columns.length !== r.cur.refColumns.length;
  if (added(s.fks).some(badFk)) {
    p.push("A new foreign key needs a table and the same number of columns on both sides.");
  }
  return p;
}
