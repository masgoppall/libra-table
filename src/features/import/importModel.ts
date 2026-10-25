// The import wizard's rules without UI: defaults, column matching, the request and the
// new table it creates.

import {
  type ColumnMap,
  type CreateTable,
  type CsvOptions,
  type FilePreview,
  type ImportFormat,
  type ImportOnError,
  type ImportProgress,
  type ImportRequest,
  type ImportSource,
  type ImportSummary,
  type ImportTransaction,
  type NewColumn,
} from "@/types";
import { plural } from "@/lib/format";

export const DEFAULT_CSV: CsvOptions = {
  delimiter: ",",
  quote: '"',
  header: true,
  encoding: "utf8",
  nullMarker: "NULL",
  emptyAsNull: true,
  skipRows: 0,
};

export interface ImportOptions {
  batchSize: number;
  onError: ImportOnError;
  transaction: ImportTransaction;
}

export const DEFAULT_OPTIONS: ImportOptions = {
  batchSize: 500,
  onError: "stop",
  transaction: "run",
};

/** A column of the table "import into new table" creates. */
export interface NewTableColumn {
  include: boolean;
  name: string;
  dataType: string;
}

export function formatOf(path: string): ImportFormat {
  return /\.(json|ndjson|jsonl)$/i.test(path) ? "json" : "csv";
}

export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** `Sales Report 2024.csv` → `sales_report_2024`. */
export function tableNameFrom(path: string): string {
  const base = fileName(path).replace(/\.[^.]+$/, "");
  const name = base
    .toLowerCase()
    .replace(/[^a-z0-9_]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return /^[a-z_]/.test(name) ? name : `t_${name || "import"}`;
}

/** Case, spaces and punctuation don't matter when matching names. */
export function normalizeName(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]/g, "");
}

/** The target column of each source column (null = skip): exact names first, then
 * names that only differ in case or punctuation. Each target is used once. */
export function autoMatch(sources: string[], targets: string[]): (string | null)[] {
  const used = new Set<string>();
  const out: (string | null)[] = sources.map((s) => {
    const t = targets.find((x) => x === s);
    if (t) used.add(t);
    return t ?? null;
  });
  return out.map((t, i) => {
    if (t) return t;
    const want = normalizeName(sources[i] ?? "");
    const match = targets.find((x) => !used.has(x) && normalizeName(x) === want);
    if (match) used.add(match);
    return match ?? null;
  });
}

/** Why the mapping can't run yet, or null. */
export function mappingProblem(targets: (string | null)[]): string | null {
  const chosen = targets.filter((t): t is string => !!t);
  if (chosen.length === 0) return "Map at least one column to import.";
  const dup = chosen.find((t, i) => chosen.indexOf(t) !== i);
  return dup ? `Column “${dup}” is mapped twice.` : null;
}

export function mappingOf(targets: (string | null)[]): ColumnMap[] {
  return targets.flatMap((target, source) => (target ? [{ source, target }] : []));
}

export function newTableColumns(preview: FilePreview): NewTableColumn[] {
  return preview.columns.map((c) => ({ include: true, name: c.name, dataType: c.suggestedType }));
}

/** Problems with the new table's name or columns, or null. */
export function newTableProblem(name: string, cols: NewTableColumn[]): string | null {
  if (!name.trim()) return "Name the new table.";
  const included = cols.filter((c) => c.include);
  if (included.length === 0) return "Include at least one column.";
  if (included.some((c) => !c.name.trim() || !c.dataType.trim())) {
    return "Every included column needs a name and a type.";
  }
  const names = included.map((c) => c.name.trim());
  const dup = names.find((n, i) => names.indexOf(n) !== i);
  return dup ? `Column “${dup}” appears twice.` : null;
}

/** For a new table, source column i goes to the column of the same row. */
export function newTableTargets(cols: NewTableColumn[]): (string | null)[] {
  return cols.map((c) => (c.include ? c.name.trim() : null));
}

export function newTableSpec(cols: NewTableColumn[]): NewColumn[] {
  return cols
    .filter((c) => c.include)
    .map((c) => ({ name: c.name.trim(), dataType: c.dataType.trim() }));
}

export function createTableChange(
  schema: string,
  name: string,
  cols: NewTableColumn[],
): CreateTable {
  const columns = newTableSpec(cols).map((c) => ({
    ...c,
    nullable: true,
    default: null,
    primaryKey: false,
    autoIncrement: false,
    unique: false,
    comment: null,
  }));
  const base = { schema, name: name.trim(), indexes: [], foreignKeys: [], comment: null };
  return { kind: "create_table", ...base, columns };
}

export function buildRequest(
  source: ImportSource,
  preview: FilePreview,
  where: { schema: string; table: string },
  targets: (string | null)[],
  options: ImportOptions,
): ImportRequest {
  return {
    source,
    sourceColumns: preview.columns.map((c) => c.name),
    schema: where.schema,
    table: where.table,
    mapping: mappingOf(targets),
    batchSize: Math.max(1, Math.round(options.batchSize) || 1),
    onError: options.onError,
    transaction: options.transaction,
  };
}

export function progressPercent(p: { bytesRead: number; totalBytes: number }): number {
  if (p.totalBytes <= 0) return 0;
  return Math.min(100, Math.round((p.bytesRead / p.totalBytes) * 100));
}

export function progressLine(p: ImportProgress): string {
  const parts = [`${plural(p.rowsRead, "row")} read`, `${plural(p.inserted, "row")} inserted`];
  if (p.skipped) parts.push(`${plural(p.skipped, "row")} skipped`);
  return parts.join(" · ");
}

export type SummaryTone = "ok" | "warn" | "error";

/** One line for the end of an import, and how it went. */
export function summaryOf(s: ImportSummary): { tone: SummaryTone; text: string } {
  const inserted = `${plural(s.inserted, "row")} imported`;
  if (s.fatal) return { tone: "error", text: `${inserted}. The import failed: ${s.fatal.message}` };
  if (s.cancelled) {
    const rolled = s.rolledBack ? " Uncommitted rows were rolled back." : "";
    return { tone: "warn", text: `Cancelled. ${inserted}.${rolled}` };
  }
  if (s.stopped) {
    const first = s.errors[0];
    const at = first ? ` at row ${first.row}: ${first.message}` : "";
    const rolled = s.rolledBack ? " Rolled back." : "";
    return { tone: "error", text: `Stopped${at}.${rolled} ${inserted}.` };
  }
  if (s.skipped) {
    return { tone: "warn", text: `${inserted}; ${plural(s.skipped, "row")} skipped.` };
  }
  return { tone: "ok", text: `${inserted}.` };
}
