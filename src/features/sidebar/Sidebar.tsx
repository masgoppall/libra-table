// Schema browser (SCH-1..SCH-4, SCH-6), laid out like TablePlus: "Items", "Queries" and
// "History" tabs, a search field, the tree of the current database (recent items, functions
// and other schema objects, tables, views) and "+" / schema menus at the bottom. Databases
// open from "Open database" (⌘K).

import { useEffect, useMemo, useRef, useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { Spinner } from "@/components/Spinner";
import { dispatch, shortcutFor } from "@/lib/actions";
import { type MatchMode, search } from "@/lib/fuzzy";
import { type Workspace } from "@/types";
import { KeyValueSidebar } from "../keyvalue/KeyValueSidebar";
import { SchemaObjectSections } from "./ObjectGroups";
import { RecentSection } from "./RecentSection";
import { SidebarFoot, SidebarSearch, type SideTab, SidebarTabs, TABS } from "./SidebarChrome";
import { SidebarHistory } from "./SidebarHistory";
import { ObjectSections, SavedQueryList, useCollapsed } from "./SidebarLists";
import { SideEmpty } from "./SideSection";
import { needsDatabase, useSidebarData } from "./useSidebarData";
import { useSidebarExtras } from "./useSidebarExtras";
import { useSidebarWidth } from "./useSidebarWidth";
import { useTableColumns } from "./useTableColumns";

function NoDatabase() {
  const keys = shortcutFor("switch-database");
  // "⌘K" reads "⌘ + K", as TablePlus writes it.
  const label = keys ? Array.from(keys).join(" + ") : "Open database";
  return (
    <SideEmpty>
      No database selected, press{" "}
      <button className="side-empty-link" onClick={() => dispatch("switch-database")}>
        {label}
      </button>{" "}
      to select a database
    </SideEmpty>
  );
}

export function Sidebar(props: {
  ws: Workspace;
  onExportTable: (schema: string, table: string) => void;
}) {
  const { ws } = props;
  const data = useSidebarData(ws);
  const size = useSidebarWidth();
  const menu = useContextMenu();
  const collapsed = useCollapsed();
  const columns = useTableColumns(ws);
  const [tab, setTab] = useState<SideTab>("items");
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<MatchMode>("fuzzy");
  const listRef = useRef<HTMLDivElement>(null);
  const asideRef = useRef<HTMLElement>(null);
  // A hidden sidebar keeps its state but takes no focus (React 18 has no `inert` prop).
  useEffect(() => {
    asideRef.current?.toggleAttribute("inert", !!ws.sidebarHidden);
  }, [ws.sidebarHidden]);
  const pick = useMemo(() => needsDatabase(ws), [ws]);
  const keyValue = ws.caps.kind === "key_value";
  const extras = useSidebarExtras(ws, data.objects, { query, mode });

  const objects = useMemo(
    () => search(extras.visibleTables, query, mode, (o) => o.name),
    [extras.visibleTables, query, mode],
  );
  const saved = useMemo(
    () => search(data.saved, query, mode, (s) => s.name),
    [data.saved, query, mode],
  );
  const sections = useMemo(() => {
    const kinds = new Set(extras.objectMatches.map((m) => m.item.kind as string));
    return ["recent", "tables", "views", "databases", ...kinds];
  }, [extras.objectMatches]);
  const collapseAll = (to: boolean) => {
    for (const k of sections) if (collapsed.isCollapsed(k) !== to) collapsed.toggle(k);
  };
  const switchTab = (t: SideTab) => {
    setTab(t);
    setQuery("");
    if (listRef.current) listRef.current.scrollTop = 0;
  };

  const items = () => {
    if (keyValue) {
      return (
        <KeyValueSidebar
          ws={ws}
          query={query}
          collapsed={collapsed.isCollapsed("databases")}
          onToggle={() => collapsed.toggle("databases")}
          menu={menu}
        />
      );
    }
    if (pick) return <NoDatabase />;
    if (data.loading || !data.objects) {
      return (
        <div className="side-loading">
          <Spinner size={16} />
        </div>
      );
    }
    return (
      <div className="side-tree" role="tree">
        <RecentSection
          ws={ws}
          matches={extras.recentMatches}
          collapsed={collapsed}
          menu={menu}
          items={extras.items}
        />
        {ws.caps.schemaObjects && (
          <SchemaObjectSections
            ws={ws}
            matches={extras.objectMatches}
            collapsed={collapsed}
            menu={menu}
            items={extras.items}
          />
        )}
        <ObjectSections
          ws={ws}
          matches={objects}
          query={query}
          collapsed={collapsed}
          menu={menu}
          columns={columns}
          items={extras.items}
          onExportTable={props.onExportTable}
        />
      </div>
    );
  };

  const style = { width: size.width, "--side-width": `${size.width}px` } as React.CSSProperties;
  return (
    <aside
      className={`sidebar${ws.sidebarHidden ? " hidden" : ""}`}
      style={style}
      aria-hidden={ws.sidebarHidden || undefined}
      ref={asideRef}
    >
      <SidebarTabs tab={tab} onTab={switchTab} />
      <SidebarSearch
        placeholder={TABS.find((t) => t.id === tab)?.placeholder ?? ""}
        query={query}
        setQuery={setQuery}
        mode={mode}
        setMode={setMode}
        menu={menu}
      />
      <div className="sidebar-list" ref={listRef}>
        {tab === "items" && items()}
        {tab === "queries" && (
          <SavedQueryList
            ws={ws}
            matches={saved}
            query={query}
            menu={menu}
            reload={data.loadSaved}
          />
        )}
        {tab === "history" && <SidebarHistory ws={ws} query={query} />}
      </div>
      <SidebarFoot ws={ws} schemas={data.schemas} menu={menu} collapseAll={collapseAll} />
      <div className="side-resize" onMouseDown={size.startResize} onDoubleClick={size.reset} />
      {menu.node}
    </aside>
  );
}
