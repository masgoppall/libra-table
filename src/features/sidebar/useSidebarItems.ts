// Recent and pinned items of a connection, kept by Rust (sidebar.json). Opening a
// table, view, object definition or saved query makes it the most recent one.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { activeTab } from "@/store";
import {
  type SidebarItem,
  type SidebarItemKind,
  type SidebarState,
  type Workspace,
  type WorkspaceTab,
} from "@/types";
import { addRecent, removeRecent, togglePin } from "./recentModel";

const EMPTY: SidebarState = { recent: [], pinned: [] };

/** What the tab shows, as a recent item; `kindOf` tells tables from views. */
export function tabItem(
  ws: Workspace,
  t: WorkspaceTab,
  kindOf: (name: string) => SidebarItemKind,
): SidebarItem | null {
  const database = t.database ?? ws.database;
  const schema = t.schema ?? ws.schema;
  if ((t.kind === "data" || t.kind === "structure") && t.table) {
    return { database, schema, kind: kindOf(t.table), name: t.table };
  }
  if (t.kind === "definition" && t.object) {
    const { kind, name, arguments: args, table } = t.object;
    return {
      database,
      schema,
      kind,
      name,
      arguments: args ?? undefined,
      table: table ?? undefined,
    };
  }
  if (t.kind === "query" && t.savedName) {
    return { database, schema: "", kind: "query", name: t.savedName };
  }
  return null;
}

export function useSidebarItems(ws: Workspace, kindOf: (name: string) => SidebarItemKind) {
  const id = ws.connectionId;
  // null until loaded, so nothing recorded early is overwritten by the load.
  const [state, setState] = useState<SidebarState | null>(null);

  useEffect(() => {
    setState(null);
    api
      .getSidebarState(id)
      .then((s) => setState(s ?? EMPTY))
      .catch(() => setState(EMPTY));
  }, [id]);

  const update = (fn: (s: SidebarState) => SidebarState) =>
    setState((prev) => {
      if (!prev) return prev;
      const next = fn(prev);
      api.saveSidebarState(id, next).catch(() => undefined);
      return next;
    });

  // The tab that becomes active is the latest one opened.
  const tab = activeTab(ws);
  const loaded = state !== null;
  useEffect(() => {
    const item = tab && tabItem(ws, tab, kindOf);
    if (!item || !loaded) return;
    update((s) => ({ ...s, recent: addRecent(s.recent, { ...item, openedAt: Date.now() }) }));
  }, [tab?.id, loaded]);

  return {
    recent: state?.recent ?? [],
    pinned: state?.pinned ?? [],
    togglePin: (item: SidebarItem) => update((s) => ({ ...s, pinned: togglePin(s.pinned, item) })),
    forget: (item: SidebarItem) => update((s) => ({ ...s, recent: removeRecent(s.recent, item) })),
    clearRecent: (database: string) =>
      update((s) => ({ ...s, recent: s.recent.filter((i) => i.database !== database) })),
  };
}

export type SidebarItems = ReturnType<typeof useSidebarItems>;
