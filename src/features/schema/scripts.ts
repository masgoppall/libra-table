// "Copy script as" for a table or view (TablePlus): its CREATE statement, DROP and
// TRUNCATE as LibraTable would run them, and SELECT / INSERT / UPDATE / DELETE templates.

import { api } from "@/api";
import { copyText } from "@/lib/clipboard";
import { qualifiedName, quoteIdent } from "@/lib/sql";
import { showError, toastSuccess } from "@/services/errors";
import { tabDb } from "@/store";
import { type ColumnInfo, type Dialect, type Workspace } from "@/types";

export type ScriptKind = "create" | "drop" | "truncate" | "select" | "insert" | "update" | "delete";

export const SCRIPT_LABELS: [ScriptKind, string][] = [
  ["create", "CREATE"],
  ["drop", "DROP"],
  ["truncate", "TRUNCATE"],
  ["select", "SELECT"],
  ["insert", "INSERT"],
  ["update", "UPDATE"],
  ["delete", "DELETE"],
];

/** Templates from the column list; `?` placeholders like TablePlus' templates. */
export function templateScript(
  kind: "select" | "insert" | "update" | "delete",
  driver: Dialect,
  schema: string,
  table: string,
  columns: Pick<ColumnInfo, "name" | "pkPosition">[],
): string {
  const t = qualifiedName(driver, schema, table);
  const q = (c: string) => quoteIdent(driver, c);
  const isKey = (c: Pick<ColumnInfo, "pkPosition">) => c.pkPosition !== null;
  const keys = columns.filter(isKey);
  const where = (keys.length ? keys : columns.slice(0, 1)).map((c) => `${q(c.name)} = ?`);
  const whereSql = where.length ? `\nWHERE ${where.join(" AND ")}` : "";
  switch (kind) {
    case "select":
      return `SELECT ${columns.map((c) => q(c.name)).join(", ") || "*"}\nFROM ${t};`;
    case "insert": {
      const cols = columns.map((c) => q(c.name)).join(", ");
      return `INSERT INTO ${t} (${cols})\nVALUES (${columns.map(() => "?").join(", ")});`;
    }
    case "update": {
      const rest = columns.filter((c) => !isKey(c));
      const set = (rest.length ? rest : columns).map((c) => `${q(c.name)} = ?`).join(",\n  ");
      return `UPDATE ${t}\nSET ${set}${whereSql};`;
    }
    case "delete":
      return `DELETE FROM ${t}${whereSql};`;
  }
}

async function script(ws: Workspace, kind: ScriptKind, schema: string, name: string) {
  const id = ws.connectionId;
  const db = tabDb(ws);
  if (kind === "drop" || kind === "truncate") {
    const change =
      kind === "drop"
        ? { kind: "drop_table" as const, schema, name }
        : { kind: "truncate_table" as const, schema, name };
    const statements = await api.previewSchemaChange(id, db, change);
    return statements.map((s) => `${s.replace(/;\s*$/, "")};`).join("\n");
  }
  const structure = await api.tableStructure(id, db, schema, name);
  if (kind === "create") return structure.ddl;
  return templateScript(kind, ws.info.dialect, schema, name, structure.columns);
}

export async function copyScript(ws: Workspace, kind: ScriptKind, schema: string, name: string) {
  try {
    await copyText(await script(ws, kind, schema, name));
    toastSuccess(`${kind.toUpperCase()} statement copied.`);
  } catch (e) {
    showError(e);
  }
}
