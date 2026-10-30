// Values for new rows made from existing ones (Duplicate) or from the clipboard (Paste).
// Pure functions: the data tab turns their results into pending inserts and edits.

import { cellText, isBinary, isDefaultMarker } from "@/lib/cells";
import { type Cell, type CellInput, type ColumnInfo, type GridValue } from "@/types";

/** Rows the grid copied last, with their real values (the text alone loses NULL vs ""). */
export interface CopiedRows {
  text: string;
  columns: string[];
  rows: Cell[][];
}

let lastCopy: CopiedRows | null = null;

export function rememberCopy(copy: CopiedRows) {
  lastCopy = copy;
}

/** The rows behind `text` when it is what the grid copied last. */
export function copiedRows(text: string): CopiedRows | null {
  return lastCopy && lastCopy.text === text ? lastCopy : null;
}

/** Columns a new row leaves to the database: generated ones, auto-increment counters and
 * primary keys with a default (a sequence, a UUID function…). */
export function takesDefault(c: ColumnInfo): boolean {
  if (c.autoIncrement) return true;
  if (/\b(VIRTUAL|STORED) GENERATED\b/i.test(c.extra ?? "")) return true;
  return c.pkPosition !== null && c.default !== null;
}

export interface NewRow {
  values: Record<string, CellInput>;
  /** Binary columns left to their default: the grid only holds a preview of them. */
  skipped: string[];
}

/** A new row with the values of `cells` (by the table's column order). */
export function rowFromCells(columns: ColumnInfo[], cells: GridValue[]): NewRow {
  const values: Record<string, CellInput> = {};
  const skipped: string[] = [];
  columns.forEach((c, i) => {
    const v = cells[i];
    if (v === undefined || isDefaultMarker(v) || takesDefault(c)) return;
    if (isBinary(v)) skipped.push(c.name);
    else values[c.name] = v;
  });
  return { values, skipped };
}

/** Tab-separated text (what "Copy row" writes) as rows of fields. */
export function parseTsv(text: string): string[][] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  while (lines.length && lines[lines.length - 1] === "") lines.pop();
  return lines.map((l) => l.split("\t"));
}

/** A pasted field as a cell value: "NULL", and an empty field in a nullable column, are NULL. */
export function fieldValue(column: ColumnInfo | undefined, text: string): CellInput {
  if (text === "NULL") return null;
  if (text === "" && (column?.nullable ?? true)) return null;
  return text;
}

/** Copied cells (or plain fields) as a value for `column`. */
function pastedValue(column: ColumnInfo, v: Cell | string, own: boolean): CellInput | undefined {
  if (!own) return fieldValue(column, v as string);
  if (isBinary(v as Cell)) return undefined;
  return v as Exclude<Cell, object>;
}

export interface PastedRows {
  rows: NewRow[];
  /** How the clipboard's columns were matched: by name (copied here) or by position. */
  matched: "name" | "position";
}

/**
 * Clipboard rows as new rows of this table, or null when they don't fit it. Rows copied
 * from a grid match columns by name (another table works too); plain text needs one field
 * per column of the table.
 */
export function pastedRows(columns: ColumnInfo[], text: string): PastedRows | null {
  const own = copiedRows(text);
  if (own) {
    const at = columns.map((c) => own.columns.indexOf(c.name));
    if (!at.some((i) => i >= 0)) return null;
    const rows = own.rows.map((cells) => build(columns, (i) => cells[at[i] as number], true));
    return { rows, matched: "name" };
  }
  const fields = parseTsv(text);
  if (!fields.length || fields.some((f) => f.length !== columns.length)) return null;
  return { rows: fields.map((f) => build(columns, (i) => f[i], false)), matched: "position" };
}

function build(
  columns: ColumnInfo[],
  valueAt: (i: number) => Cell | string | undefined,
  own: boolean,
): NewRow {
  const values: Record<string, CellInput> = {};
  const skipped: string[] = [];
  columns.forEach((c, i) => {
    const v = valueAt(i);
    if (v === undefined || takesDefault(c)) return;
    const value = pastedValue(c, v, own);
    if (value === undefined) skipped.push(c.name);
    else values[c.name] = value;
  });
  return { values, skipped };
}

/** The clipboard as a block of field texts for pasting into cells; null skips a cell
 * (binary values, of which the grid only has a preview). */
export function pastedFields(text: string): (string | null)[][] {
  const own = copiedRows(text);
  if (!own) return parseTsv(text);
  return own.rows.map((r) =>
    r.map((v) => (isBinary(v) ? null : v === null ? "NULL" : cellText(v))),
  );
}
