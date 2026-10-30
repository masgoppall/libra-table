// Opening a table in the current tab, with Back / Forward through what the tab showed
// (TablePlus' "< >" in the tab bar). Pure, like workspaceModel.

import { type NewTab, type TabPlace, type Workspace, type WorkspaceTab } from "@/types";

const HISTORY_DEPTH = 30;

type TabsChange = Partial<Pick<Workspace, "tabs" | "activeTabId">>;

/** Kinds that show one object and can be replaced by another. */
const NAVIGABLE = new Set(["data", "structure", "definition"]);

export const isNavigable = (t: WorkspaceTab | undefined) => !!t && NAVIGABLE.has(t.kind);

export function placeOf(t: WorkspaceTab): TabPlace {
  const { kind, title, database, schema, table, object } = t;
  return { kind, title, database, schema, table, object };
}

/** The tab's other fields reset: a filter or a draft of the old object don't carry over. */
function show(t: WorkspaceTab, place: TabPlace): WorkspaceTab {
  return {
    id: t.id,
    back: t.back,
    forward: t.forward,
    ...place,
  };
}

function replaceTab(ws: Workspace, tab: WorkspaceTab) {
  return { tabs: ws.tabs.map((t) => (t.id === tab.id ? tab : t)), activeTabId: tab.id };
}

/** Shows `to` in the tab, remembering what it showed for Back. */
export function navigateTab(ws: Workspace, tabId: string, to: NewTab): TabsChange {
  const t = ws.tabs.find((x) => x.id === tabId);
  if (!t) return {};
  const back = [...(t.back ?? []), placeOf(t)].slice(-HISTORY_DEPTH);
  const place = { ...placeOf({ ...to, id: t.id }), database: to.database ?? ws.database };
  return replaceTab(ws, { ...show(t, place), ...to, id: t.id, back, forward: [] });
}

/** One step back (dir -1) or forward (dir 1) in the tab's history. */
export function stepTab(ws: Workspace, tabId: string, dir: -1 | 1): TabsChange {
  const t = ws.tabs.find((x) => x.id === tabId);
  const from = dir < 0 ? t?.back : t?.forward;
  const to = from?.[from.length - 1];
  if (!t || !from || !to) return {};
  const rest = from.slice(0, -1);
  const other = [...((dir < 0 ? t.forward : t.back) ?? []), placeOf(t)];
  const next = show(t, to);
  return replaceTab(
    ws,
    dir < 0 ? { ...next, back: rest, forward: other } : { ...next, back: other, forward: rest },
  );
}
