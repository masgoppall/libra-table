// Renaming a database (Postgres: ALTER DATABASE … RENAME TO); its tabs and the rail
// follow the new name.

import { runSchemaChange } from "@/services/schemaChange";
import { store } from "@/store";
import { type Workspace } from "@/types";

export const canRenameDatabase = (ws: Workspace) =>
  ws.caps.createDatabase && ws.info.dialect === "postgres";

export async function renameDatabase(ws: Workspace, from: string) {
  const r = await store.get().dialog({
    kind: "prompt",
    title: `Rename database ${from}`,
    label: "New name",
    initial: from,
    okLabel: "Continue",
  });
  const to = r.ok ? (r.value?.trim() ?? "") : "";
  if (!to || to === from) return;
  const ok = await runSchemaChange(
    ws.connectionId,
    null,
    { kind: "rename_database", from, to },
    { title: `Rename database ${from}`, okLabel: "Rename", success: `Renamed to "${to}".` },
  );
  if (!ok) return;
  const current = store.workspace(ws.connectionId);
  if (!current) return;
  const rename = (d: string | undefined) => (d === from ? to : d);
  store.get().updateWorkspace(ws.connectionId, {
    database: rename(current.database) ?? to,
    databases: current.databases?.map((d) => rename(d) ?? d),
    tabs: current.tabs.map((t) => ({ ...t, database: rename(t.database) })),
  });
  store.get().refresh(ws.connectionId);
}
