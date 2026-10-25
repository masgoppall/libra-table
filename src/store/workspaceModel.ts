// Pure rules for workspaces and their tabs. The store applies them; tests call them directly.

import { sameObject } from "@/domain/objects";
import { defaultSchema } from "@/domain/schema";
import { uid } from "@/lib/uid";
import {
  type Capabilities,
  type ConnectInfo,
  type Db,
  type NewTab,
  type SavedWorkspace,
  type Workspace,
  type WorkspaceTab,
} from "@/types";

type TabsPatch = Pick<Workspace, "tabs" | "activeTabId">;

/** The database a tab works in, for API calls (null = the connection's own). */
export function tabDb(ws: Workspace, tab?: { database?: string }): Db {
  const db = tab?.database ?? ws.database;
  return db && db !== ws.info.database ? db : null;
}

export function activeTab(ws: Workspace): WorkspaceTab | undefined {
  return ws.tabs.find((t) => t.id === ws.activeTabId);
}

/** Data, structure and definition tabs of the same object are reused, not opened twice. */
function isReusable(t: WorkspaceTab, wanted: WorkspaceTab): boolean {
  if (t.kind !== wanted.kind || t.kind === "query" || t.kind === "new-table") return false;
  const sameTable = t.table === wanted.table && sameObject(t.object, wanted.object);
  return t.database === wanted.database && t.schema === wanted.schema && sameTable;
}

export function openTab(ws: Workspace, tab: NewTab, reuse: boolean): TabsPatch {
  const full: WorkspaceTab = { ...tab, database: tab.database ?? ws.database, id: uid() };
  const same = reuse ? ws.tabs.find((t) => isReusable(t, full)) : undefined;
  if (same) return { tabs: ws.tabs, activeTabId: same.id };
  // A new tab opens next to the current one.
  const at = ws.tabs.findIndex((t) => t.id === ws.activeTabId);
  const tabs = [...ws.tabs];
  tabs.splice(at < 0 ? tabs.length : at + 1, 0, full);
  return { tabs, activeTabId: full.id };
}

/** Keeps the active tab if it survived, else picks a neighbour. */
function keepActive(ws: Workspace, tabs: WorkspaceTab[], removedAt = tabs.length): string | null {
  if (tabs.some((t) => t.id === ws.activeTabId)) return ws.activeTabId;
  return tabs[Math.min(removedAt, tabs.length - 1)]?.id ?? null;
}

export function closeTab(ws: Workspace, tabId: string): TabsPatch {
  const at = ws.tabs.findIndex((t) => t.id === tabId);
  const tabs = ws.tabs.filter((t) => t.id !== tabId);
  return { tabs, activeTabId: keepActive(ws, tabs, at) };
}

export function closeTabsWhere(ws: Workspace, match: (t: WorkspaceTab) => boolean): TabsPatch {
  const tabs = ws.tabs.filter((t) => !match(t));
  return { tabs, activeTabId: keepActive(ws, tabs) };
}

export function closeTabsAfter(ws: Workspace, tabId: string): TabsPatch {
  const at = ws.tabs.findIndex((t) => t.id === tabId);
  const tabs = ws.tabs.slice(0, at + 1);
  return { tabs, activeTabId: keepActive(ws, tabs) ?? tabId };
}

export function moveTab(ws: Workspace, tabId: string, toIndex: number): WorkspaceTab[] {
  const tab = ws.tabs.find((t) => t.id === tabId);
  if (!tab) return ws.tabs;
  const tabs = ws.tabs.filter((t) => t.id !== tabId);
  tabs.splice(Math.max(0, Math.min(toIndex, tabs.length)), 0, tab);
  return tabs;
}

/** The tab `dir` steps away from the active one, wrapping around. */
export function cycleTab(ws: Workspace, dir: 1 | -1): string | null {
  if (ws.tabs.length < 2) return ws.activeTabId;
  const i = ws.tabs.findIndex((t) => t.id === ws.activeTabId);
  return ws.tabs[(i + dir + ws.tabs.length) % ws.tabs.length]?.id ?? null;
}

/** ⌘1…⌘8 pick that tab; ⌘9 is the last tab, as in browsers. */
export function tabByNumber(ws: Workspace, n: number): WorkspaceTab | undefined {
  return n === 9 ? ws.tabs[ws.tabs.length - 1] : ws.tabs[n - 1];
}

/** The view to show after closing `id`: its right neighbour, else the left, else home. */
export function viewAfterClose(order: string[], id: string, current: string): string {
  if (current !== id) return current;
  const at = order.indexOf(id);
  return order[at + 1] ?? order[at - 1] ?? "home";
}

/** A fresh workspace, picking up the tabs saved from the last session. */
export function restoreWorkspace(
  info: ConnectInfo,
  caps: Capabilities,
  saved: SavedWorkspace | undefined,
): Workspace {
  let database = info.database;
  let schema = defaultSchema(info.dialect, info.database);
  let tabs: WorkspaceTab[] = [];
  let activeTabId: string | null = null;
  if (saved) {
    // Tabs saved before databases were per tab belong to the connection's database.
    tabs = (saved.tabs ?? []).map((t) => ({ ...t, database: t.database ?? info.database }));
    activeTabId = saved.activeTabId ?? tabs[0]?.id ?? null;
    if (saved.database !== undefined && info.dialect !== "sqlite") database = saved.database;
    if (saved.schema) schema = saved.schema;
  }
  if (info.dialect === "mysql") schema = database;
  const base = { connectionId: info.connectionId, info, caps, refreshKey: 0 };
  return { ...base, database, schema, tabs, activeTabId };
}

/** What is saved for the next start: everything but "New table" drafts. */
export function toSaved(ws: Workspace): SavedWorkspace {
  const tabs = ws.tabs.filter((t) => t.kind !== "new-table");
  const activeTabId = tabs.some((t) => t.id === ws.activeTabId)
    ? ws.activeTabId
    : (tabs[0]?.id ?? null);
  return { database: ws.database, schema: ws.schema, tabs, activeTabId };
}
