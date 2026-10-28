// The sidebar's frame: the Items / Queries / History tabs, the search field with its
// options, and the "+" and schema menus at the bottom.

import { Check } from "lucide-react";
import { type ContextMenuApi, type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { shortcutFor } from "@/lib/actions";
import { type MatchMode } from "@/lib/fuzzy";
import { store } from "@/store";
import { type Workspace } from "@/types";
import { openMetrics } from "../import/openTools";
import { createDatabaseFlow } from "../schema/flows";
import { newQueryTab, newTableTab } from "../workspace/tabs";
import { schemaMenu } from "./sidebarMenus";
import { hasSchemaPicker, needsDatabase } from "./useSidebarData";

export type SideTab = "items" | "queries" | "history";

export const TABS: { id: SideTab; label: string; placeholder: string }[] = [
  { id: "items", label: "Items", placeholder: "Search for item..." },
  { id: "queries", label: "Queries", placeholder: "Search for query..." },
  { id: "history", label: "History", placeholder: "Search for history..." },
];

const check = (on: boolean) => (on ? <Check size={14} /> : <span className="icon-gap" />);

export function SidebarTabs(props: { tab: SideTab; onTab: (t: SideTab) => void }) {
  return (
    <div className="side-tabs" role="tablist">
      {TABS.map((t) => (
        <button
          key={t.id}
          role="tab"
          aria-selected={props.tab === t.id}
          className={`side-tab${props.tab === t.id ? " active" : ""}`}
          onClick={() => props.onTab(t.id)}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

export function SidebarSearch(props: {
  placeholder: string;
  query: string;
  setQuery: (q: string) => void;
  mode: MatchMode;
  setMode: (m: MatchMode) => void;
  menu: ContextMenuApi;
}) {
  const options = (el: HTMLElement) =>
    props.menu.openAt(el, [
      {
        label: "Fuzzy match",
        icon: check(props.mode === "fuzzy"),
        onClick: () => props.setMode("fuzzy"),
      },
      {
        label: "Contains",
        icon: check(props.mode === "contains"),
        onClick: () => props.setMode("contains"),
      },
    ]);
  return (
    <div className="side-search-row">
      <label className="side-search">
        <svg width="13" height="13" viewBox="0 0 15 15" aria-hidden className="side-search-icon">
          <circle cx="6.2" cy="6.2" r="5" fill="none" stroke="currentColor" strokeWidth="1.4" />
          <path d="m9.9 9.9 4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
        <input
          placeholder={props.placeholder}
          value={props.query}
          onChange={(e) => props.setQuery(e.target.value)}
          onKeyDown={(e) => e.key === "Escape" && props.setQuery("")}
        />
      </label>
      <button
        className="side-filter"
        title="Search options"
        aria-label="Search options"
        onClick={(e) => options(e.currentTarget)}
      >
        <svg width="17" height="15" viewBox="0 0 17 15" aria-hidden>
          <g stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" fill="none">
            <path d="M1 2.5h15M1 7.5h15M1 12.5h15" />
          </g>
          <g stroke="currentColor" strokeWidth="1.2" fill="var(--side-bg)">
            <circle cx="11.5" cy="2.5" r="1.9" />
            <circle cx="5" cy="7.5" r="1.9" />
            <circle cx="9.5" cy="12.5" r="1.9" />
          </g>
        </svg>
      </button>
    </div>
  );
}

/** "+": what can be created here. */
function createMenu(ws: Workspace): MenuItem[] {
  const id = ws.connectionId;
  const { addTab } = store.get();
  const items: MenuItem[] = [];
  if (ws.caps.createTable && !needsDatabase(ws)) {
    items.push({ label: "New table", onClick: () => addTab(id, newTableTab(ws.schema)) });
  }
  if (ws.caps.query) items.push({ label: "New query", onClick: () => addTab(id, newQueryTab()) });
  if (ws.caps.createDatabase) {
    items.push({ label: "New database…", onClick: () => createDatabaseFlow(ws) });
  }
  if (ws.caps.metrics) items.push({ label: "New metrics board", onClick: () => openMetrics(ws) });
  return items;
}

/** "⌄": the schema to browse (Postgres, SQLite, engines with schemas) and the tree. */
function viewMenu(
  ws: Workspace,
  schemas: string[],
  collapseAll: (collapsed: boolean) => void,
): MenuItem[] {
  const set = (schema: string) => store.get().updateWorkspace(ws.connectionId, { schema });
  const items: MenuItem[] = [];
  if (hasSchemaPicker(ws) && schemas.length) {
    for (const s of schemas) {
      items.push({ label: s, icon: check(s === ws.schema), onClick: () => set(s) });
    }
    if (ws.info.dialect === "postgres") items.push(SEPARATOR, ...schemaMenu(ws));
    items.push(SEPARATOR);
  }
  items.push(
    { label: "Expand all", onClick: () => collapseAll(false) },
    { label: "Collapse all", onClick: () => collapseAll(true) },
    SEPARATOR,
    {
      label: "Refresh",
      hint: shortcutFor("refresh"),
      onClick: () => store.get().refresh(ws.connectionId),
    },
  );
  return items;
}

export function SidebarFoot(props: {
  ws: Workspace;
  schemas: string[];
  menu: ContextMenuApi;
  collapseAll: (collapsed: boolean) => void;
}) {
  const { ws, menu } = props;
  const create = createMenu(ws);
  const schema = hasSchemaPicker(ws) && props.schemas.length > 0 ? ws.schema : null;
  return (
    <div className="sidebar-foot">
      <button
        className="side-foot-btn"
        title="New…"
        aria-label="New"
        disabled={create.length === 0}
        onClick={(e) => menu.openAt(e.currentTarget, create)}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden>
          <path d="M6 1v10M1 6h10" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
      </button>
      <button
        className="side-foot-btn schema"
        title={schema ? "Schema" : "Sidebar options"}
        aria-label={schema ? "Schema" : "Sidebar options"}
        onClick={(e) =>
          menu.openAt(e.currentTarget, viewMenu(ws, props.schemas, props.collapseAll))
        }
      >
        {schema && <span className="side-foot-schema">{schema}</span>}
        <svg width="10" height="6" viewBox="0 0 10 6" aria-hidden>
          <path
            d="M1 1l4 4 4-4"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </button>
    </div>
  );
}
