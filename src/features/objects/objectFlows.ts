// Opening, dropping, refreshing and resetting schema objects. Every change previews the
// exact SQL Rust will run and asks first (services/schemaChange); production
// connections get the red confirmation.

import { api } from "@/api";
import { KIND_LABEL, objectRef, objectTitle, sameObject } from "@/domain/objects";
import { showError } from "@/services/errors";
import { runSchemaChange } from "@/services/schemaChange";
import { store, tabDb } from "@/store";
import { type SchemaObjectRef, type Workspace } from "@/types";
import { newQueryTab, tableTab } from "../workspace/tabs";
import { definitionTab } from "./objectKinds";

const refresh = (ws: Workspace) => store.get().refresh(ws.connectionId);

/** Materialized views open their rows (like views); the rest their definition. */
export function openObject(ws: Workspace, o: SchemaObjectRef, schema = ws.schema) {
  const id = ws.connectionId;
  if (o.kind === "materialized_view") {
    store.get().addTab(id, tableTab("data", o.name, { schema }), { reuse: true });
  } else {
    store.get().addTab(id, definitionTab(o, { schema }), { reuse: true });
  }
}

export function openDefinition(ws: Workspace, o: SchemaObjectRef, schema = ws.schema) {
  store.get().addTab(ws.connectionId, definitionTab(o, { schema }), { reuse: true });
}

/** A query tab with the CREATE OR REPLACE (or DROP + CREATE) script of the object. */
export async function editInQueryTab(ws: Workspace, o: SchemaObjectRef, schema = ws.schema) {
  try {
    const def = await api.objectDefinition(ws.connectionId, tabDb(ws), schema, objectRef(o));
    const tab = newQueryTab(`${def.editScript}\n`, { title: `Edit · ${o.name}`, schema });
    store.get().addTab(ws.connectionId, tab);
  } catch (e) {
    showError(e);
  }
}

/** Postgres can cascade to dependent objects; null when the user cancels. */
async function askCascade(ws: Workspace, o: SchemaObjectRef): Promise<boolean | null> {
  if (ws.info.driver !== "postgres") return false;
  const what = KIND_LABEL[o.kind];
  const r = await store.get().dialog({
    kind: "confirm",
    title: `Drop ${what} ${o.name}`,
    message:
      `Drop ${what} "${objectTitle(o)}"? ` +
      `With "cascade", objects that depend on it are dropped too.`,
    checkbox: "Cascade",
    okLabel: "Continue",
    danger: true,
  });
  return r.ok ? !!r.checked : null;
}

export async function dropObject(ws: Workspace, o: SchemaObjectRef, schema = ws.schema) {
  const cascade = await askCascade(ws, o);
  if (cascade === null) return;
  const what = KIND_LABEL[o.kind];
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "drop_object", schema, object: objectRef(o), cascade },
    {
      title: `Drop ${what} ${o.name}`,
      okLabel: `Drop ${what}`,
      danger: true,
      typeToConfirm: o.name,
      success: `Dropped "${o.name}".`,
    },
  );
  if (!ok) return;
  store
    .get()
    .closeTabsWhere(
      ws.connectionId,
      (t) => t.database === ws.database && t.schema === schema && sameObject(t.object, o),
    );
  refresh(ws);
}

export async function refreshMaterializedView(
  ws: Workspace,
  name: string,
  opts: { concurrently?: boolean; schema?: string } = {},
): Promise<boolean> {
  const how = opts.concurrently ? " concurrently" : "";
  const ok = await runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    {
      kind: "refresh_materialized_view",
      schema: opts.schema ?? ws.schema,
      name,
      concurrently: !!opts.concurrently,
    },
    { title: `Refresh ${name}${how}`, okLabel: "Refresh", success: `Refreshed "${name}".` },
  );
  if (ok) refresh(ws);
  return ok;
}

/** Asks for the next value, then previews `setval`. True when it ran. */
export async function resetSequence(
  ws: Workspace,
  name: string,
  opts: { initial?: string; schema?: string } = {},
): Promise<boolean> {
  const r = await store.get().dialog({
    kind: "prompt",
    title: `Reset sequence ${name}`,
    message: "nextval() returns this value next.",
    label: "Next value",
    initial: opts.initial ?? "1",
    okLabel: "Continue",
  });
  if (!r.ok) return false;
  const value = Number((r.value ?? "").trim());
  if (!Number.isSafeInteger(value)) {
    store.get().toast("error", `"${r.value ?? ""}" is not a whole number.`);
    return false;
  }
  return runSchemaChange(
    ws.connectionId,
    tabDb(ws),
    { kind: "reset_sequence", schema: opts.schema ?? ws.schema, name, value },
    {
      title: `Reset ${name}`,
      okLabel: "Reset sequence",
      success: `"${name}" restarts at ${value}.`,
    },
  );
}
