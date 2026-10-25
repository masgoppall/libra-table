// Identifier and literal quoting for SQL the UI writes itself (copy as INSERT, SELECT …).

import { type Cell, type Dialect } from "@/types";
import { isBinary } from "./cells";

export function quoteIdent(driver: Dialect, name: string): string {
  if (driver === "mysql") return "`" + name.replace(/`/g, "``") + "`";
  return '"' + name.replace(/"/g, '""') + '"';
}

/** `"schema"."table"` from a dotted name or from parts. */
export function qualifiedName(driver: Dialect, ...parts: string[]): string {
  return parts.map((p) => quoteIdent(driver, p)).join(".");
}

export function sqlLiteral(driver: Dialect, v: Cell): string {
  if (v === null) return "NULL";
  if (typeof v === "number") return String(v);
  if (typeof v === "boolean") return v ? "TRUE" : "FALSE";
  if (isBinary(v)) return driver === "postgres" ? `'\\x${v.$binary}'::bytea` : `X'${v.$binary}'`;
  const escaped = driver === "mysql" ? v.replace(/\\/g, "\\\\") : v;
  return "'" + escaped.replace(/'/g, "''") + "'";
}

export function selectAllSql(driver: Dialect, schema: string, table: string, limit = 100): string {
  return `SELECT * FROM ${qualifiedName(driver, schema, table)} LIMIT ${limit};`;
}

/** Statements that change what the schema browser shows. */
export function isDdl(sql: string): boolean {
  return /^\s*(create|alter|drop|rename|truncate)\b/i.test(sql);
}
