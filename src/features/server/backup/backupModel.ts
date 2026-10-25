// Pure rules of the backup and restore dialogs: defaults, file names and filters, and
// how progress reads.

import { formatBytes } from "@/lib/format";
import { type BackupRequest, type Dialect, type DumpFormat, type RestoreRequest } from "@/types";

export function defaultBackup(): BackupRequest {
  return {
    path: "",
    format: "plain",
    content: "all",
    schemas: [],
    tables: [],
    noOwner: false,
    noPrivileges: false,
    clean: false,
    createDatabase: false,
    singleTransaction: true,
    routines: true,
    triggers: true,
    events: false,
  };
}

export function defaultRestore(): RestoreRequest {
  return { path: "", stopOnError: true, singleTransaction: false, clean: false, noOwner: false };
}

export function backupExtension(driver: Dialect, format: DumpFormat): string {
  if (driver === "sqlite") return "db";
  return driver === "postgres" && format === "custom" ? "dump" : "sql";
}

/** `shop-2026-10-25.sql`. */
export function backupFileName(database: string, ext: string, now = new Date()): string {
  const day = now.toISOString().slice(0, 10);
  const base =
    database
      .split("/")
      .pop()
      ?.replace(/\.[^.]*$/, "") || "backup";
  return `${base}-${day}.${ext}`;
}

/** File types the restore picker offers. */
export function restoreFilters(driver: Dialect): { name: string; extensions: string[] }[] {
  const sql = { name: "SQL script", extensions: ["sql"] };
  if (driver !== "postgres") return [sql];
  return [{ name: "SQL script or pg_dump archive", extensions: ["sql", "dump", "backup"] }, sql];
}

/** "1.2 MB written", "3.0 MB of 12.0 MB (25%)". */
export function progressText(bytes: number, total: number | null, verb: string): string {
  if (!total) return `${formatBytes(bytes)} ${verb}`;
  const pct = Math.min(100, Math.round((bytes / total) * 100));
  return `${formatBytes(bytes)} of ${formatBytes(total)} (${pct}%)`;
}

/** Keeps the last `max` lines of a tool's output. */
export function appendLine(lines: string[], line: string, max = 200): string[] {
  const next = [...lines, line];
  return next.length > max ? next.slice(next.length - max) : next;
}

export function toggleName(names: string[], name: string, on: boolean): string[] {
  return on ? [...names.filter((n) => n !== name), name] : names.filter((n) => n !== name);
}
