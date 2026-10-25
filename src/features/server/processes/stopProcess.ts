// Cancel a query or kill a connection: show the exact statement, confirm, then run it.

import { api } from "@/api";
import { isProduction } from "@/domain/connection";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type ProcessAction, type ProcessInfo, type Workspace } from "@/types";
import { stopWording } from "./processModel";

/** True when the statement ran. */
export async function stopProcess(
  ws: Workspace,
  p: ProcessInfo,
  action: ProcessAction,
): Promise<boolean> {
  const id = ws.connectionId;
  const conn = store.connection(id);
  const w = stopWording(ws.info.dialect, p, action);
  try {
    const statements = await api.previewProcessAction(id, p.id, action);
    const prod = isProduction(conn);
    const r = await store.get().dialog({
      kind: "confirm",
      title: prod ? `Production: ${w.title}` : w.title,
      message: w.message,
      details: statements,
      okLabel: w.okLabel,
      danger: true,
      typeToConfirm: w.typeToConfirm,
    });
    if (!r.ok) return false;
    await api.runProcessAction(id, p.id, action, true);
    toastSuccess(
      action === "cancel" ? `Query of ${p.id} cancelled.` : `Connection ${p.id} closed.`,
    );
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}
