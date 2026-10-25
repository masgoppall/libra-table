// Every schema change shows the exact SQL Rust will run (db/ddl) before it runs.

import { api } from "@/api";
import { isProduction } from "@/domain/connection";
import { plural } from "@/lib/format";
import { store } from "@/store";
import { type Db, type SchemaChange } from "@/types";
import { showError, toastSuccess } from "./errors";
import { writeDialogOptions } from "./safeMode";

export interface SchemaChangeOptions {
  title: string;
  okLabel: string;
  danger?: boolean;
  typeToConfirm?: string;
  success?: string;
}

/**
 * Previews a schema change, asks for confirmation with the SQL, then applies it.
 * Returns true when it ran. Production connections get the red confirmation.
 */
export async function runSchemaChange(
  connectionId: string,
  db: Db,
  change: SchemaChange,
  opts: SchemaChangeOptions,
): Promise<boolean> {
  const conn = store.connection(connectionId);
  const prod = isProduction(conn);
  const guard = writeDialogOptions(connectionId);
  try {
    const statements = await api.previewSchemaChange(connectionId, db, change);
    const where = conn ? ` on ${conn.name}` : "";
    const r = await store.get().dialog({
      kind: "confirm",
      title: prod ? `Production: ${opts.title}` : opts.title,
      message: `${plural(statements.length, "statement")} will run${where}.`,
      details: statements,
      okLabel: opts.okLabel,
      danger: opts.danger || prod || guard.danger,
      typeToConfirm: opts.typeToConfirm ?? guard.typeToConfirm,
    });
    if (!r.ok) return false;
    await api.applySchemaChange(connectionId, db, change, true);
    if (opts.success) toastSuccess(opts.success);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}
