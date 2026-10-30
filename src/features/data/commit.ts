// Commit pending edits: preview the statements, confirm, run them in one transaction.

import { api } from "@/api";
import { isProduction } from "@/domain/connection";
import { plural } from "@/lib/format";
import { confirmDiscard } from "@/services/confirm";
import { showError, toastSuccess } from "@/services/errors";
import { writeDialogOptions } from "@/services/safeMode";
import { store } from "@/store";
import { type Change, type Db } from "@/types";

export interface CommitTarget {
  connectionId: string;
  db: Db;
  schema: string;
  table: string;
}

/** True when committed. */
export async function commitChanges(t: CommitTarget, changes: Change[]): Promise<boolean> {
  if (!changes.length) return false;
  const conn = store.connection(t.connectionId);
  const prod = isProduction(conn);
  const guard = writeDialogOptions(t.connectionId);
  try {
    const preview = await api.previewChanges(t.connectionId, t.db, t.schema, t.table, changes);
    const r = await store.get().dialog({
      kind: "confirm",
      title: prod ? `Production: ${conn?.name}` : "Commit changes",
      message: `${plural(preview.length, "statement")} will run in one transaction.`,
      details: preview,
      okLabel: prod ? "Commit on production" : "Commit",
      danger: prod || guard.danger,
      typeToConfirm: guard.typeToConfirm,
    });
    if (!r.ok) return false;
    const target = { schema: t.schema, table: t.table };
    // The dialog above is the confirmation safe mode asks for.
    const res = await api.commitChanges(t.connectionId, t.db, target, changes, true);
    toastSuccess(`Committed ${plural(res.affected, "row change")}.`);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}

/** Commit (⌘S) and refresh (asking before dropping uncommitted edits) of a data tab. */
export function tableSaving(
  target: CommitTarget,
  changes: () => Change[] | null,
  dirty: number,
  load: () => void,
) {
  const commit = async () => {
    const list = changes();
    if (list && (await commitChanges(target, list))) load();
  };
  const reload = async () => {
    const wording = { doing: "Refreshing", verb: "refresh", kind: "uncommitted" };
    if (await confirmDiscard(dirty, wording)) load();
  };
  return { commit, reload };
}
