// The sections of the sidebar list: databases, tables, views and saved queries
// (recent items and other schema objects are in RecentSection / ObjectGroups).

import { Database, FileCode2, Plus } from "lucide-react";
import { useState } from "react";
import { type ContextMenuApi } from "@/components/ContextMenu";
import { Highlight } from "@/components/Highlight";
import { withShortcut } from "@/lib/actions";
import { type SearchResult } from "@/lib/fuzzy";
import { activeTab, store } from "@/store";
import { type ObjectInfo, type SavedQuery, type Workspace } from "@/types";
import { openSavedQuery } from "../query/savedQueries";
import { createDatabaseFlow } from "../schema/flows";
import { newTableTab, tableTab } from "../workspace/tabs";
import { ObjectItem } from "./ObjectItem";
import { pinMenuItem, sidebarItem } from "./pins";
import { isPinned, pinnedFirst } from "./recentModel";
import { databaseMenu, objectMenu, savedQueryMenu, tablesMenu } from "./sidebarMenus";
import { SideHint, SideSection } from "./SideSection";
import { type SidebarItems } from "./useSidebarItems";
import { type TableColumns } from "./useTableColumns";

/** "databases", "tables", "views", "queries", "recent" or a schema object kind. */
type SectionKey = string;

/** Which sections are collapsed. "Recent" starts collapsed: it repeats names shown below. */
export function useCollapsed() {
  const [collapsed, setCollapsed] = useState<Partial<Record<SectionKey, boolean>>>({
    recent: true,
  });
  const toggle = (k: SectionKey) => setCollapsed((c) => ({ ...c, [k]: !c[k] }));
  return { isCollapsed: (k: SectionKey) => !!collapsed[k], toggle };
}

export type Collapsed = ReturnType<typeof useCollapsed>;

function AddButton(props: { title: string; onClick: () => void }) {
  return (
    <button className="icon-btn" title={props.title} onClick={props.onClick}>
      <Plus size={13} />
    </button>
  );
}

export function DatabaseSection(props: {
  ws: Workspace;
  matches: SearchResult<string>[];
  collapsed: Collapsed;
  menu: ContextMenuApi;
}) {
  const { ws } = props;
  const open = (d: string) => d !== ws.database && store.get().switchDatabase(ws.connectionId, d);
  return (
    <SideSection
      label="Databases"
      count={props.matches.length}
      collapsed={props.collapsed.isCollapsed("databases")}
      onToggle={() => props.collapsed.toggle("databases")}
      action={
        ws.caps.createDatabase && (
          <AddButton title="New database" onClick={() => createDatabaseFlow(ws)} />
        )
      }
    >
      {props.matches.map(({ item: d, match }) => (
        <div
          key={d}
          className={`side-item db${d === ws.database ? " current" : ""}`}
          onClick={() => open(d)}
          onContextMenu={(e) => props.menu.open(e, databaseMenu(ws, d))}
          title={d === ws.database ? `${d} (open)` : `Open ${d}`}
        >
          <Database size={13} className="side-icon db" />
          <span className="label">
            <Highlight text={d} indexes={match.indexes} />
          </span>
          {d === ws.info.database && <span className="side-tag">default</span>}
        </div>
      ))}
    </SideSection>
  );
}

function isOpenTable(ws: Workspace, name: string): boolean {
  const t = activeTab(ws);
  if (!t || (t.kind !== "data" && t.kind !== "structure")) return false;
  const sameDb = (t.database ?? ws.database) === ws.database;
  return t.table === name && t.schema === ws.schema && sameDb;
}

export function ObjectSections(props: {
  ws: Workspace;
  matches: SearchResult<ObjectInfo>[];
  query: string;
  collapsed: Collapsed;
  menu: ContextMenuApi;
  columns: TableColumns;
  items: SidebarItems;
  onExportTable: (schema: string, table: string) => void;
}) {
  const { ws, columns: cols, items } = props;
  const id = ws.connectionId;
  const itemOf = (o: ObjectInfo) => sidebarItem(ws, o.kind, o.name);
  // Pinned tables and views float to the top of their section.
  const ofKind = (kind: ObjectInfo["kind"]) =>
    pinnedFirst(
      props.matches.filter((r) => r.item.kind === kind),
      items.pinned,
      (r) => itemOf(r.item),
    );
  const tables = ofKind("table");
  const views = ofKind("view");
  const openData = (name: string) =>
    store.get().addTab(id, tableTab("data", name, { schema: ws.schema }), { reuse: true });
  const newTable = () => store.get().addTab(id, newTableTab(ws.schema));

  const render = ({ item: o, match }: SearchResult<ObjectInfo>) => (
    <ObjectItem
      key={o.name}
      object={o}
      indexes={match.indexes}
      active={isOpenTable(ws, o.name)}
      expanded={!!cols.expanded[o.name]}
      columns={cols.columns[o.name]}
      onOpen={() => openData(o.name)}
      onToggle={() => void cols.toggle(o.name)}
      pinned={isPinned(items.pinned, itemOf(o))}
      onMenu={(e) => {
        const pin = pinMenuItem(items, itemOf(o));
        props.menu.open(e, objectMenu(ws, o, props.onExportTable, [pin]));
      }}
    />
  );

  return (
    <>
      <SideSection
        label={ws.caps.kind === "document" ? "Collections" : "Tables"}
        count={tables.length}
        collapsed={props.collapsed.isCollapsed("tables")}
        onToggle={() => props.collapsed.toggle("tables")}
        onMenu={(e) => props.menu.open(e, tablesMenu(ws))}
        action={
          ws.caps.createTable && (
            <AddButton title={withShortcut("New table", "new-table")} onClick={newTable} />
          )
        }
      >
        {tables.length ? (
          tables.map(render)
        ) : (
          <SideHint>{props.query ? "No tables match." : "No tables yet."}</SideHint>
        )}
      </SideSection>
      {views.length > 0 && (
        <SideSection
          label="Views"
          count={views.length}
          collapsed={props.collapsed.isCollapsed("views")}
          onToggle={() => props.collapsed.toggle("views")}
        >
          {views.map(render)}
        </SideSection>
      )}
    </>
  );
}

export function SavedQuerySection(props: {
  ws: Workspace;
  matches: SearchResult<SavedQuery>[];
  query: string;
  collapsed: Collapsed;
  menu: ContextMenuApi;
  reload: () => void;
}) {
  const { ws } = props;
  const active = activeTab(ws)?.savedName;
  const empty = props.query ? "No saved queries match." : "Save a query with ⌘S to keep it here.";
  return (
    <SideSection
      label="Saved queries"
      count={props.matches.length}
      collapsed={props.collapsed.isCollapsed("queries")}
      onToggle={() => props.collapsed.toggle("queries")}
    >
      {props.matches.length ? (
        props.matches.map(({ item: s, match }) => (
          <div
            key={s.name}
            className={`side-item${active === s.name ? " active" : ""}`}
            onClick={() => void openSavedQuery(ws.connectionId, s.name)}
            onContextMenu={(e) => props.menu.open(e, savedQueryMenu(ws, s, props.reload))}
          >
            <FileCode2 size={13} className="side-icon query" />
            <span className="label">
              <Highlight text={s.name} indexes={match.indexes} />
            </span>
          </div>
        ))
      ) : (
        <SideHint>{empty}</SideHint>
      )}
    </SideSection>
  );
}
