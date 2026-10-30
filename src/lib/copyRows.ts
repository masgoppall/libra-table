// Rows → clipboard text in the formats of the grid's "Copy as" menu (GRD-6).

import { type Cell, type Dialect } from "@/types";
import { cellText } from "./cells";
import { qualifiedName, quoteIdent, sqlLiteral } from "./sql";

export type CopyFormat = "tsv" | "csv" | "json" | "sql" | "markdown" | "html";

export interface CopyOptions {
  driver: Dialect;
  /** Dotted table name for INSERT statements. */
  table?: string;
  header?: boolean;
}

function csvField(text: string, delimiter: string): string {
  const needsQuotes = /["\r\n]/.test(text) || text.includes(delimiter);
  return needsQuotes ? '"' + text.replace(/"/g, '""') + '"' : text;
}

function toTsv(columns: string[], rows: Cell[][], header: boolean): string {
  const clean = (v: Cell) => cellText(v).replace(/\t/g, " ").replace(/\r?\n/g, " ");
  const lines = rows.map((r) => r.map(clean).join("\t"));
  return (header ? [columns.join("\t"), ...lines] : lines).join("\n");
}

function toCsv(columns: string[], rows: Cell[][], header: boolean): string {
  const line = (values: string[]) => values.map((v) => csvField(v, ",")).join(",");
  const lines = rows.map((r) => line(r.map(cellText)));
  return (header ? [line(columns), ...lines] : lines).join("\n");
}

function toJson(columns: string[], rows: Cell[][]): string {
  const objects = rows.map((r) => Object.fromEntries(columns.map((c, i) => [c, r[i]])));
  return JSON.stringify(objects, null, 2);
}

function toInserts(columns: string[], rows: Cell[][], opts: CopyOptions): string {
  const table = opts.table ? qualifiedName(opts.driver, ...opts.table.split(".")) : "table_name";
  const cols = columns.map((c) => quoteIdent(opts.driver, c)).join(", ");
  return rows
    .map((r) => {
      const values = r.map((v) => sqlLiteral(opts.driver, v)).join(", ");
      return `INSERT INTO ${table} (${cols}) VALUES (${values});`;
    })
    .join("\n");
}

function toMarkdown(columns: string[], rows: Cell[][]): string {
  const esc = (s: string) => s.replace(/\|/g, "\\|").replace(/\r?\n/g, " ");
  const line = (values: string[]) => `| ${values.join(" | ")} |`;
  const head = line(columns.map(esc));
  const sep = line(columns.map(() => "---"));
  const body = rows.map((r) => line(r.map((v) => esc(v === null ? "NULL" : cellText(v)))));
  return [head, sep, ...body].join("\n");
}

function toHtml(columns: string[], rows: Cell[][]): string {
  const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  const line = (tag: string, values: string[]) =>
    `    <tr>${values.map((v) => `<${tag}>${esc(v)}</${tag}>`).join("")}</tr>`;
  const body = rows.map((r) =>
    line(
      "td",
      r.map((v) => (v === null ? "NULL" : cellText(v))),
    ),
  );
  return ["<table>", "  <thead>", line("th", columns), "  </thead>", "  <tbody>", ...body]
    .concat(["  </tbody>", "</table>"])
    .join("\n");
}

export function copyRows(
  format: CopyFormat,
  columns: string[],
  rows: Cell[][],
  opts: CopyOptions,
): string {
  const header = opts.header ?? false;
  switch (format) {
    case "tsv":
      return toTsv(columns, rows, header);
    case "csv":
      return toCsv(columns, rows, header);
    case "json":
      return toJson(columns, rows);
    case "sql":
      return toInserts(columns, rows, opts);
    case "markdown":
      return toMarkdown(columns, rows);
    case "html":
      return toHtml(columns, rows);
  }
}
