// Starting an import. Into an existing table it goes through safe mode (production asks
// first). Into a new table the CREATE TABLE and the INSERT shape are previewed together
// and confirmed once, like every schema change; then the table is created and filled.

import { api } from "@/api";
import { isProduction } from "@/domain/connection";
import { plural } from "@/lib/format";
import { withSafeMode } from "@/services/safeMode";
import { store } from "@/store";
import { type CreateTable, type Db, type ImportRequest, type NewColumn } from "@/types";

export interface ImportTarget {
  connectionId: string;
  db: Db;
}

export function startExisting(t: ImportTarget, jobId: string, req: ImportRequest) {
  const name = store.connection(t.connectionId)?.name ?? "";
  return withSafeMode(name, (confirmed) =>
    api.startImport(t.connectionId, t.db, jobId, req, confirmed).then(() => true),
  ).then((ok) => ok === true);
}

export async function startNewTable(
  t: ImportTarget,
  jobId: string,
  change: CreateTable,
  req: ImportRequest,
  columns: NewColumn[],
): Promise<boolean> {
  const prod = isProduction(store.connection(t.connectionId));
  const ddl = await api.previewSchemaChange(t.connectionId, t.db, change);
  const insert = await api.previewImport(t.connectionId, t.db, req, columns);
  const r = await store.get().dialog({
    kind: "confirm",
    title: prod ? "Production: Import into new table" : "Import into new table",
    message:
      `${plural(ddl.length, "statement")} create “${change.name}”, ` +
      "then the rows are inserted.",
    details: [...ddl, ...insert],
    okLabel: "Create table & import",
    danger: prod,
  });
  if (!r.ok) return false;
  await api.applySchemaChange(t.connectionId, t.db, change, true);
  store.get().refresh(t.connectionId);
  await api.startImport(t.connectionId, t.db, jobId, req, true);
  return true;
}
