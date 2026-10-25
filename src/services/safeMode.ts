// Safe mode (SAFE-2): Rust refuses statements the connection's safe mode wants confirmed
// until the UI resends them with `confirmed: true`.

import { isProduction } from "@/domain/connection";
import { writeConfirmation } from "@/domain/safeMode";
import { plural } from "@/lib/format";
import { store } from "@/store";
import { type AppError, type ConfirmationData, type ConnectionView } from "@/types";

function describe(statements: ConfirmationData["statements"]): string[] {
  return statements.map((s) =>
    s.understood ? s.sql : `${s.sql}\n-- (could not be analyzed; treated as a write)`,
  );
}

function confirmTitle(connName: string, conn: ConnectionView | undefined): string {
  return isProduction(conn) ? `Production: ${connName}` : `Confirm on ${connName}`;
}

/** Runs `fn(false)`; if safe mode asks for confirmation, shows the statements and retries. */
export async function withSafeMode<T>(
  connName: string,
  fn: (confirmed: boolean) => Promise<T>,
): Promise<T | null> {
  try {
    return await fn(false);
  } catch (err) {
    const e = err as AppError;
    if (e.kind !== "confirmation_required") throw e;
    const statements = e.data?.statements ?? [];
    const conn = store.get().connections.find((c) => c.name === connName);
    const r = await store.get().dialog({
      kind: "confirm",
      title: confirmTitle(connName, conn),
      message: `${plural(statements.length, "statement")} need your confirmation to run.`,
      details: describe(statements),
      okLabel: isProduction(conn) ? "Run on production" : "Run",
      danger: true,
      typeToConfirm: e.data?.level === "safe" ? connName : undefined,
    });
    if (!r.ok) return null;
    return await fn(true);
  }
}

/** What a dialog for a change on this connection adds: red styling, typing the name. */
export function writeDialogOptions(connectionId: string): {
  danger: boolean;
  typeToConfirm?: string;
} {
  const conn = store.connection(connectionId);
  const level = conn ? writeConfirmation(conn) : "none";
  return {
    danger: level !== "none",
    typeToConfirm: level === "safe" ? conn?.name : undefined,
  };
}
