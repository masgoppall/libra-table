// Database, schema and table operations from the sidebar, the palette and the menu.
// Each shows the exact SQL Rust will run before it runs (services/schemaChange).

import { api } from "@/api";
import { isProduction } from "@/domain/connection";
import { showError, toastInfo, toastSuccess } from "@/services/errors";
import { runSchemaChange } from "@/services/schemaChange";
import { store, tabDb } from "@/store";
import { type AppError, type Workspace, type WorkspaceTab } from "@/types";
import { tableTab } from "../workspace/tabs";

/** Postgres refuses to drop a database other sessions use (SQLSTATE 55006). */
const OBJECT_IN_USE = "55006";

const refresh = (ws: Workspace) => store.get().refresh(ws.connectionId);

function closeTabs(ws: Workspace, match: (t: WorkspaceTab) => boolean) {
  store.get().closeTabsWhere(ws.connectionId, match);
}

const inTable = (ws: Workspace, schema: string, table: string) => (t: WorkspaceTab) =>
  t.database === ws.database && t.schema === schema && t.table === table;

async function prompt(
  title: string,
  label: string,
  extra: { initial?: string; placeholder?: string; checkbox?: string; checked?: boolean } = {},
) {
  const r = await store
    .get()
    .dialog({ kind: "prompt", title, label, okLabel: "Continue", ...extra });
  return { value: r.ok ? (r.value?.trim() ?? "") : "", checked: r.ok && !!r.checked };
}

export function createDatabaseFlow(ws: Workspace) {
  if (ws.info.dialect === "sqlite") {
    toastInfo(
      "An SQLite database is a file.",
      "Create a new file from New connection → SQLite → Create new file.",
    );
    return;
  }
  store.get().setCreateDatabaseFor(ws.connectionId);
}

async function confirmDropDatabase(ws: Workspace, name: string): Promise<boolean> {
  const statements = await api.previewSchemaChange(ws.connectionId, null, {
    kind: "drop_database",
    name,
  });
  const prod = isProduction(store.connection(ws.connectionId));
  const r = await store.get().dialog({
    kind: "confirm",
    title: `${prod ? "Production: " : ""}Drop database ${name}`,
    message: `Every table, view and row in "${name}" is deleted. This can't be undone.`,
    details: statements,
    okLabel: "Drop database",
    danger: true,
    typeToConfirm: name,
  });
  return r.ok;
}

/** Drops; if other sessions hold the database, offers to end them (Postgres 13+). */
async function applyDropDatabase(ws: Workspace, name: string): Promise<boolean> {
  try {
    await api.applySchemaChange(ws.connectionId, null, { kind: "drop_database", name }, true);
    return true;
  } catch (e) {
    const err = e as AppError;
    if (err.kind !== "query" || err.data?.code !== OBJECT_IN_USE) throw e;
  }
  const again = await store.get().dialog({
    kind: "confirm",
    title: `Disconnect sessions and drop ${name}?`,
    message:
      `Other sessions are connected to "${name}". ` +
      "Postgres 13 and later can end them and drop it anyway.",
    okLabel: "Disconnect & drop",
    danger: true,
  });
  if (!again.ok) return false;
  const change = { kind: "drop_database" as const, name, force: true };
  await api.applySchemaChange(ws.connectionId, null, change, true);
  return true;
}

export async function dropDatabase(ws: Workspace, name: string) {
  if (name === ws.info.database) {
    store
      .get()
      .toast(
        "error",
        `LibraTable is connected to "${name}" itself, so it can't drop it.`,
        "Set another database in the connection settings, reconnect, then drop this one.",
      );
    return;
  }
  try {
    if (!(await confirmDropDatabase(ws, name))) return;
    if (!(await applyDropDatabase(ws, name))) return;
  } catch (e) {
    showError(e);
    return;
  }
  toastSuccess(`Dropped database "${name}".`);
  closeTabs(ws, (t) => t.database === name);
  if (store.workspace(ws.connectionId)?.database === name) {
    store.get().switchDatabase(ws.connectionId, ws.info.database);
  }
  refresh(ws);
}

export async function createSchema(ws: Workspace) {
  const { value: name } = await prompt("New schema", "Schema name", {
    placeholder: "e.g. analytics",
  });
  if (!name) return;
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "create_schema", name },
    {
      title: `Create schema ${name}`,
      okLabel: "Create schema",
      success: `Created schema "${name}".`,
    },
  );
  if (!ok) return;
  store.get().updateWorkspace(ws.connectionId, { schema: name });
  refresh(ws);
}

export async function dropSchema(ws: Workspace, name: string) {
  const r = await store.get().dialog({
    kind: "confirm",
    title: `Drop schema ${name}`,
    message: `Drop schema "${name}"? With "cascade", every table and view in it is dropped too.`,
    checkbox: "Cascade (drop everything in it)",
    okLabel: "Continue",
    danger: true,
  });
  if (!r.ok) return;
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "drop_schema", name, cascade: !!r.checked },
    {
      title: `Drop schema ${name}`,
      okLabel: "Drop schema",
      danger: true,
      typeToConfirm: name,
      success: `Dropped schema "${name}".`,
    },
  );
  if (!ok) return;
  closeTabs(ws, (t) => t.database === ws.database && t.schema === name);
  if (ws.schema === name) store.get().updateWorkspace(ws.connectionId, { schema: "public" });
  refresh(ws);
}

export async function renameTable(ws: Workspace, schema: string, table: string) {
  const { value: to } = await prompt(`Rename ${table}`, "New name", { initial: table });
  if (!to || to === table) return;
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "rename_table", schema, from: table, to },
    { title: `Rename ${table}`, okLabel: "Rename", success: `Renamed to "${to}".` },
  );
  if (!ok) return;
  const current = store.workspace(ws.connectionId);
  if (current) {
    const renamed = inTable(ws, schema, table);
    const tabs = current.tabs.map((t) => (renamed(t) ? { ...t, table: to, title: to } : t));
    store.get().updateWorkspace(ws.connectionId, { tabs });
  }
  refresh(ws);
}

export async function duplicateTable(ws: Workspace, schema: string, table: string) {
  const { value: to, checked: withData } = await prompt(`Duplicate ${table}`, "Name of the copy", {
    initial: `${table}_copy`,
    checkbox: "Copy the rows too",
    checked: true,
  });
  if (!to) return;
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "duplicate_table", schema, from: table, to, withData },
    { title: `Duplicate ${table}`, okLabel: "Duplicate", success: `Created "${to}".` },
  );
  if (!ok) return;
  refresh(ws);
  store.get().addTab(ws.connectionId, tableTab("data", to, { schema }), { reuse: true });
}

export async function truncateTable(ws: Workspace, schema: string, table: string) {
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "truncate_table", schema, name: table },
    {
      title: `Empty ${table}`,
      okLabel: "Delete all rows",
      danger: true,
      typeToConfirm: table,
      success: `Emptied "${table}".`,
    },
  );
  if (ok) refresh(ws);
}

/** Postgres asks about CASCADE first; null when the user cancels. */
async function askCascade(ws: Workspace, kind: string, table: string): Promise<boolean | null> {
  if (ws.info.dialect !== "postgres") return false;
  const r = await store.get().dialog({
    kind: "confirm",
    title: `Drop ${kind} ${table}`,
    message:
      `Drop ${kind} "${table}"? With "cascade", views and foreign keys ` +
      "that depend on it are dropped too.",
    checkbox: "Cascade",
    okLabel: "Continue",
    danger: true,
  });
  return r.ok ? !!r.checked : null;
}

export async function dropTable(
  ws: Workspace,
  schema: string,
  table: string,
  kind: "table" | "view",
) {
  const cascade = await askCascade(ws, kind, table);
  if (cascade === null) return;
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "drop_table", schema, name: table, cascade },
    {
      title: `Drop ${kind} ${table}`,
      okLabel: `Drop ${kind}`,
      danger: true,
      typeToConfirm: kind === "table" ? table : undefined,
      success: `Dropped "${table}".`,
    },
  );
  if (!ok) return;
  closeTabs(ws, inTable(ws, schema, table));
  refresh(ws);
}
