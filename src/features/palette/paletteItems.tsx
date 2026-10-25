// What the palettes list: ⌘P tables, views, saved queries and actions; ⌘K databases.

import { Command, Database, Eye, FileCode2, Plus, Table2 } from "lucide-react";
import { type ReactNode } from "react";
import { api } from "@/api";
import { type ActionId, dispatch, SHORTCUTS, shortcutFor, UNBOUND_ACTIONS } from "@/lib/actions";
import { showError } from "@/services/errors";
import { store, tabDb } from "@/store";
import { type Workspace } from "@/types";
import { objectPaletteItems } from "../objects/paletteObjects";
import { openSavedQuery } from "../query/savedQueries";
import { createDatabaseFlow } from "../schema/flows";
import { SERVER_ACTIONS, serverActionAvailable } from "../server/serverActions";
import { tableTab } from "../workspace/tabs";

export interface PaletteItem {
  key: string;
  group: string;
  label: string;
  detail?: string;
  icon: ReactNode;
  hint?: string;
  /** `alt` is Shift+Enter (structure instead of data). */
  run: (alt: boolean) => void;
}

const ACTIONS: ActionId[] = [
  "new-query",
  "new-table",
  "new-database",
  "history",
  "refresh",
  "export",
  "export-tables",
  "import-data",
  "import-new-table",
  "import-sql",
  "open-diagram",
  "open-metrics",
  "settings",
  "shortcuts",
  "disconnect",
  "home",
  "toggle-sidebar",
  "explain",
  "format-sql",
];

function actionLabel(a: ActionId): string {
  if (a === "new-database") return "New database";
  return UNBOUND_ACTIONS[a] ?? SHORTCUTS.find((s) => s.id === a)?.label ?? a;
}

function actionItems(ws: Workspace): PaletteItem[] {
  const server = SERVER_ACTIONS.filter((a) => serverActionAvailable(ws, a));
  return [...ACTIONS, ...server].map((a) => ({
    key: `act:${a}`,
    group: "Actions",
    label: actionLabel(a),
    icon: <Command size={14} />,
    hint: shortcutFor(a),
    run: () => dispatch(a),
  }));
}

export async function databaseItems(ws: Workspace): Promise<PaletteItem[]> {
  const dbs = await api.listDatabases(ws.connectionId).catch((e) => {
    showError(e);
    return [] as string[];
  });
  const items: PaletteItem[] = dbs.map((d) => ({
    key: `db:${d}`,
    group: "Databases",
    label: d,
    detail:
      d === ws.database ? "current" : d === ws.info.database ? "connection default" : undefined,
    icon: <Database size={14} />,
    run: () => store.get().switchDatabase(ws.connectionId, d),
  }));
  if (ws.info.driver !== "sqlite") {
    items.push({
      key: "new-db",
      group: "Actions",
      label: "New database…",
      icon: <Plus size={14} />,
      run: () => createDatabaseFlow(ws),
    });
  }
  return items;
}

export async function anythingItems(ws: Workspace): Promise<PaletteItem[]> {
  const id = ws.connectionId;
  const [objects, saved, others] = await Promise.all([
    api.listObjects(id, tabDb(ws), ws.schema).catch(() => []),
    api.listSavedQueries(id).catch(() => []),
    objectPaletteItems(ws),
  ]);
  const where = { schema: ws.schema };
  const tables: PaletteItem[] = objects.map((o) => ({
    key: `obj:${o.name}`,
    group: o.kind === "view" ? "Views" : "Tables",
    label: o.name,
    detail: `${ws.database || ws.info.database}.${ws.schema}`,
    icon: o.kind === "view" ? <Eye size={14} /> : <Table2 size={14} />,
    hint: "⇧↵ structure",
    run: (alt) =>
      store.get().addTab(id, tableTab(alt ? "structure" : "data", o.name, where), { reuse: true }),
  }));
  const queries: PaletteItem[] = saved.map((s) => ({
    key: `saved:${s.name}`,
    group: "Saved queries",
    label: s.name,
    icon: <FileCode2 size={14} />,
    run: () => void openSavedQuery(id, s.name),
  }));
  return [...tables, ...others, ...queries, ...actionItems(ws)];
}
