// Comparing two schemas (TablePlus' "Sync database", structure only): which tables the
// target lacks, which it has extra, and which columns differ; then the statements that
// make the target match the source. Pure: structures come in, SQL text goes out.

import { qualifiedName, quoteIdent } from "@/lib/sql";
import { type ColumnInfo, type Dialect, type TableStructure } from "@/types";

export interface ColumnChange {
  name: string;
  from: ColumnInfo;
  to: ColumnInfo;
}

export interface TableDiff {
  table: string;
  add: ColumnInfo[];
  drop: string[];
  alter: ColumnChange[];
}

export interface SchemaDiff {
  /** Only in the source: created in the target. */
  create: string[];
  /** Only in the target: dropped when chosen. */
  drop: string[];
  changed: TableDiff[];
}

const norm = (s: string | null | undefined) => (s ?? "").trim().toLowerCase();

const sameColumn = (a: ColumnInfo, b: ColumnInfo) =>
  norm(a.dataType) === norm(b.dataType) &&
  a.nullable === b.nullable &&
  norm(a.default) === norm(b.default);

export function diffTable(table: string, source: TableStructure, target: TableStructure) {
  const theirs = new Map(target.columns.map((c) => [c.name, c]));
  const ours = new Set(source.columns.map((c) => c.name));
  const d: TableDiff = { table, add: [], drop: [], alter: [] };
  for (const c of source.columns) {
    const t = theirs.get(c.name);
    if (!t) d.add.push(c);
    else if (!sameColumn(c, t)) d.alter.push({ name: c.name, from: t, to: c });
  }
  d.drop = target.columns.filter((c) => !ours.has(c.name)).map((c) => c.name);
  return d;
}

export function diffSchemas(
  source: Record<string, TableStructure>,
  target: Record<string, TableStructure>,
): SchemaDiff {
  const names = (r: Record<string, TableStructure>) => Object.keys(r).sort();
  const changed: TableDiff[] = [];
  for (const t of names(source)) {
    const s = source[t];
    const other = target[t];
    if (!s || !other) continue;
    const d = diffTable(t, s, other);
    if (d.add.length || d.drop.length || d.alter.length) changed.push(d);
  }
  return {
    create: names(source).filter((t) => !target[t]),
    drop: names(target).filter((t) => !source[t]),
    changed,
  };
}

function columnSql(driver: Dialect, c: ColumnInfo): string {
  const parts = [quoteIdent(driver, c.name), c.dataType];
  if (!c.nullable) parts.push("NOT NULL");
  if (c.default !== null && c.default !== "") parts.push(`DEFAULT ${c.default}`);
  return parts.join(" ");
}

function alterSql(driver: Dialect, t: string, ch: ColumnChange): string[] {
  const col = quoteIdent(driver, ch.name);
  if (driver === "mysql") return [`ALTER TABLE ${t} MODIFY COLUMN ${columnSql(driver, ch.to)};`];
  if (driver === "sqlite") {
    return [`-- SQLite can't alter column ${ch.name}: rebuild ${t} in the structure tab.`];
  }
  const out: string[] = [];
  if (norm(ch.from.dataType) !== norm(ch.to.dataType)) {
    out.push(`ALTER TABLE ${t} ALTER COLUMN ${col} TYPE ${ch.to.dataType};`);
  }
  if (ch.from.nullable !== ch.to.nullable) {
    out.push(`ALTER TABLE ${t} ALTER COLUMN ${col} ${ch.to.nullable ? "DROP" : "SET"} NOT NULL;`);
  }
  if (norm(ch.from.default) !== norm(ch.to.default)) {
    out.push(
      ch.to.default
        ? `ALTER TABLE ${t} ALTER COLUMN ${col} SET DEFAULT ${ch.to.default};`
        : `ALTER TABLE ${t} ALTER COLUMN ${col} DROP DEFAULT;`,
    );
  }
  return out;
}

export interface ScriptOptions {
  driver: Dialect;
  /** Schema of the target tables. */
  schema: string;
  /** CREATE statements of the source tables, by name. */
  ddl: Record<string, string>;
  /** Schema (MySQL: database) the source's CREATE statements name; renamed to `schema`. */
  sourceSchema: string;
  /** Drop tables and columns the source doesn't have. */
  dropExtra: boolean;
  /** Only these tables (all when absent). */
  only?: Set<string>;
}

/** The statements that bring the target to the source's structure. */
export function syncScript(diff: SchemaDiff, o: ScriptOptions): string {
  const picked = (t: string) => !o.only || o.only.has(t);
  const t = (name: string) => qualifiedName(o.driver, o.schema, name);
  const out: string[] = [];
  for (const name of diff.create.filter(picked)) {
    const ddl = o.ddl[name]
      ?.trim()
      .replaceAll(qualifiedName(o.driver, o.sourceSchema, name), t(name));
    out.push(ddl ? (ddl.endsWith(";") ? ddl : `${ddl};`) : `-- No CREATE statement for ${name}.`);
  }
  for (const d of diff.changed.filter((x) => picked(x.table))) {
    for (const c of d.add)
      out.push(`ALTER TABLE ${t(d.table)} ADD COLUMN ${columnSql(o.driver, c)};`);
    for (const ch of d.alter) out.push(...alterSql(o.driver, t(d.table), ch));
    if (o.dropExtra) {
      for (const c of d.drop) {
        out.push(`ALTER TABLE ${t(d.table)} DROP COLUMN ${quoteIdent(o.driver, c)};`);
      }
    }
  }
  if (o.dropExtra) for (const name of diff.drop.filter(picked)) out.push(`DROP TABLE ${t(name)};`);
  return out.join("\n");
}
