// The sidebar's tables and views sections and its saved queries (recent items and other
// schema objects are in RecentSection / ObjectGroups; databases are in "Open database").

import { FileCode2, Folder, Plus } from "lucide-react";
import { type ReactNode, useState } from "react";
import { type ContextMenuApi } from "@/components/ContextMenu";
import { Highlight } from "@/components/Highlight";
import { shortcutFor, withShortcut } from "@/lib/actions";
import { type SearchResult } from "@/lib/fuzzy";
import { activeTab, store } from "@/store";
import { type ObjectInfo, type SavedQuery, type Workspace } from "@/types";
import { openSavedQuery, splitSavedName } from "../query/savedQueries";
import { newTableTab, tableTab } from "../workspace/tabs";
import { ObjectItem } from "./ObjectItem";
import { pinMenuItem, sidebarItem } from "./pins";
import { isPinned, pinnedFirst } from "./recentModel";
import { objectMenu, savedQueryMenu, tablesMenu } from "./sidebarMenus";
import { Disclosure, SideEmpty, SideHint, SideSection } from "./SideSection";
import { type SidebarItems } from "./useSidebarItems";
import { type TableColumns } from "./useTableColumns";

/** "databases", "tables", "views", "recent" or a schema object kind. */
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
  const openData = (name: string, newTab = false) =>
    store.get().addTab(id, tableTab("data", name, { schema: ws.schema }), { reuse: true, newTab });
  const newTable = () => store.get().addTab(id, newTableTab(ws.schema));

  const render = ({ item: o, match }: SearchResult<ObjectInfo>) => (
    <ObjectItem
      key={o.name}
      object={o}
      indexes={match.indexes}
      active={isOpenTable(ws, o.name)}
      expanded={!!cols.expanded[o.name]}
      columns={cols.columns[o.name]}
      onOpen={(newTab) => openData(o.name, newTab)}
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

/** The "Queries" tab: saved queries of the connection (QRY-7). */
export function SavedQueryList(props: {
  ws: Workspace;
  matches: SearchResult<SavedQuery>[];
  query: string;
  menu: ContextMenuApi;
  reload: () => void;
}) {
  const { ws } = props;
  const active = activeTab(ws)?.savedName;
  if (!props.matches.length) {
    const save = shortcutFor("save") ?? "Save";
    return (
      <SideEmpty>
        {props.query
          ? `No saved queries match “${props.query}”.`
          : `No saved queries yet. Save a query (${save}) to keep it here.`}
      </SideEmpty>
    );
  }
  const row = ({ item: s, match }: SearchResult<SavedQuery>, inFolder: boolean) => {
    const [folder, base] = splitSavedName(s.name);
    const shift = folder === null ? 0 : folder.length + 1;
    return (
      <div
        key={s.name}
        className={`side-item${inFolder ? " in-folder" : ""}${active === s.name ? " active" : ""}`}
        onClick={() => void openSavedQuery(ws.connectionId, s.name)}
        onContextMenu={(e) => props.menu.open(e, savedQueryMenu(ws, s, props.reload))}
        title={s.name}
      >
        <FileCode2 size={13} className="side-icon query" />
        <span className="label">
          <Highlight
            text={base}
            indexes={match.indexes.map((i) => i - shift).filter((i) => i >= 0)}
          />
        </span>
      </div>
    );
  };
  const { loose, folders } = byFolder(props.matches);
  return (
    <div className="side-flat">
      {folders.map(([folder, items]) => (
        <QueryFolder key={folder} name={folder} count={items.length} open={!!props.query}>
          {items.map((m) => row(m, true))}
        </QueryFolder>
      ))}
      {loose.map((m) => row(m, false))}
    </div>
  );
}

function byFolder(matches: SearchResult<SavedQuery>[]) {
  const loose: SearchResult<SavedQuery>[] = [];
  const folders = new Map<string, SearchResult<SavedQuery>[]>();
  for (const m of matches) {
    const [folder] = splitSavedName(m.item.name);
    if (folder === null) loose.push(m);
    else folders.set(folder, [...(folders.get(folder) ?? []), m]);
  }
  const sorted = [...folders.entries()].sort(([a], [b]) => a.localeCompare(b));
  return { loose, folders: sorted };
}

/** A folder of saved queries; open while searching. */
function QueryFolder(props: { name: string; count: number; open: boolean; children: ReactNode }) {
  const [expanded, setExpanded] = useState(true);
  const open = props.open || expanded;
  return (
    <>
      <div className="side-item query-folder" onClick={() => setExpanded(!expanded)}>
        <span className="side-caret">
          <Disclosure open={open} />
        </span>
        <Folder size={13} className="side-icon folder" />
        <span className="label">{props.name}</span>
        <span className="side-count">{props.count}</span>
      </div>
      {open && props.children}
    </>
  );
}
