// User changes show the exact SQL Rust will run (passwords masked) before they run.

import { api } from "@/api";
import { isProduction } from "@/domain/connection";
import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type Db, type Grantee, type UserChange, type Workspace } from "@/types";
import { granteeLabel } from "./userModel";

export interface UserChangeOptions {
  title: string;
  okLabel: string;
  danger?: boolean;
  typeToConfirm?: string;
  success?: string;
}

/** Previews, confirms (red on production) and applies a change; true when it ran. */
export async function runUserChange(
  ws: Workspace,
  db: Db,
  change: UserChange,
  opts: UserChangeOptions,
): Promise<boolean> {
  const id = ws.connectionId;
  const conn = store.connection(id);
  const prod = isProduction(conn);
  try {
    const statements = await api.previewUserChange(id, db, change);
    if (statements.length === 0) {
      toastSuccess("Nothing to change.");
      return false;
    }
    const where = conn ? ` on ${conn.name}` : "";
    const r = await store.get().dialog({
      kind: "confirm",
      title: prod ? `Production: ${opts.title}` : opts.title,
      message: `${plural(statements.length, "statement")} will run${where}.`,
      details: statements,
      okLabel: opts.okLabel,
      danger: opts.danger || prod,
      typeToConfirm: opts.typeToConfirm,
    });
    if (!r.ok) return false;
    await api.applyUserChange(id, db, change, true);
    if (opts.success) toastSuccess(opts.success);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}

/** Asks for the new password once (it is never stored), then sets it. */
export async function setPassword(ws: Workspace, db: Db, grantee: Grantee): Promise<boolean> {
  const label = granteeLabel(grantee);
  const r = await store.get().dialog({
    kind: "prompt",
    title: `Password for ${label}`,
    label: "New password",
    secret: true,
    okLabel: "Continue",
  });
  if (!r.ok || r.value === undefined) return false;
  const change: UserChange = { kind: "set_password", grantee, password: r.value };
  return runUserChange(ws, db, change, {
    title: "Set password",
    okLabel: "Set password",
    success: `Password of ${label} changed.`,
  });
}

/** Drops a role or account after its name is typed. */
export function dropGrantee(ws: Workspace, db: Db, grantee: Grantee): Promise<boolean> {
  const label = granteeLabel(grantee);
  const change: UserChange =
    ws.info.driver === "postgres"
      ? { kind: "drop_role", name: grantee.name }
      : { kind: "drop_user", user: grantee.name, host: grantee.host ?? "%" };
  return runUserChange(ws, db, change, {
    title: ws.info.driver === "postgres" ? "Drop role" : "Drop user",
    okLabel: "Drop",
    danger: true,
    typeToConfirm: grantee.name,
    success: `${label} dropped.`,
  });
}
