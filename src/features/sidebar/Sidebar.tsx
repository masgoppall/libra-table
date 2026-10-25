// Schema browser (SCH-1..SCH-4, SCH-6): databases of the server, schemas, recent items,
// tables, views, other schema objects (functions, triggers…) and saved queries (QRY-7),
// with the operations in context menus.

import { Gauge, History, RefreshCw, SquareTerminal } from "lucide-react";
import { useMemo, useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { BusyIcon, Spinner } from "@/components/Spinner";
import { withShortcut } from "@/lib/actions";
import { type MatchMode, search } from "@/lib/fuzzy";
import { store } from "@/store";
import { type Workspace } from "@/types";
import { openMetrics } from "../import/openTools";
import { KeyValueSidebar } from "../keyvalue/KeyValueSidebar";
import { historyTab, newQueryTab } from "../workspace/tabs";
import { SchemaObjectSections } from "./ObjectGroups";
import { RecentSection } from "./RecentSection";
import { DatabaseSection, ObjectSections, SavedQuerySection, useCollapsed } from "./SidebarLists";
import { SidebarTop } from "./SidebarTop";
import { SideHint } from "./SideSection";
import { needsDatabase, useSidebarData } from "./useSidebarData";
import { useSidebarExtras } from "./useSidebarExtras";
import { useSidebarWidth } from "./useSidebarWidth";
import { useTableColumns } from "./useTableColumns";

function SidebarFoot({ ws, loading }: { ws: Workspace; loading: boolean }) {
  const id = ws.connectionId;
  const { addTab, refresh } = store.get();
  return (
    <div className="sidebar-foot">
      {ws.caps.query && (
        <>
          <button
            className="btn sm ghost"
            title={withShortcut("New query", "new-query")}
            onClick={() => addTab(id, newQueryTab())}
          >
            <SquareTerminal size={13} /> Query
          </button>
          <button
            className="btn sm ghost"
            title={withShortcut("Query history", "history")}
            onClick={() => addTab(id, historyTab(), { reuse: true })}
          >
            <History size={13} /> History
          </button>
        </>
      )}
      {ws.caps.metrics && (
        <button className="btn sm ghost" title="Metrics boards" onClick={() => openMetrics(ws)}>
          <Gauge size={13} /> Metrics
        </button>
      )}
      <span className="spacer" />
      <button
        className="btn sm icon ghost"
        title={withShortcut("Refresh", "refresh")}
        onClick={() => refresh(id)}
      >
        <BusyIcon busy={loading} icon={<RefreshCw size={13} />} />
      </button>
    </div>
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
  const [query, setQuery] = useState("");
  const [mode, setMode] = useState<MatchMode>("fuzzy");
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
  const databases = useMemo(
    () => search(data.databases, query, mode, (d) => d),
    [data.databases, query, mode],
  );

  if (ws.sidebarHidden) return null;
  return (
    <aside className="sidebar" style={{ width: size.width }}>
      <SidebarTop
        ws={ws}
        schemas={data.schemas}
        menu={menu}
        search={{ query, setQuery, mode, setMode }}
      />
      <div className="sidebar-list">
        {keyValue && (
          <>
            <KeyValueSidebar
              ws={ws}
              query={query}
              collapsed={collapsed.isCollapsed("databases")}
              onToggle={() => collapsed.toggle("databases")}
              menu={menu}
            />
            <SavedQuerySection
              ws={ws}
              matches={saved}
              query={query}
              collapsed={collapsed}
              menu={menu}
              reload={data.loadSaved}
            />
          </>
        )}
        {ws.caps.switchDatabase && !keyValue && data.databases.length > 0 && (
          <DatabaseSection ws={ws} matches={databases} collapsed={collapsed} menu={menu} />
        )}
        {data.loading && (
          <div className="side-item faint">
            <Spinner size={12} /> Loading…
          </div>
        )}
        {pick && !data.loading && <SideHint>Choose a database above to see its tables.</SideHint>}
        {data.objects && !pick && !keyValue && (
          <>
            <RecentSection
              ws={ws}
              matches={extras.recentMatches}
              collapsed={collapsed}
              menu={menu}
              items={extras.items}
            />
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
            {ws.caps.schemaObjects && (
              <SchemaObjectSections
                ws={ws}
                matches={extras.objectMatches}
                collapsed={collapsed}
                menu={menu}
                items={extras.items}
              />
            )}
            <SavedQuerySection
              ws={ws}
              matches={saved}
              query={query}
              collapsed={collapsed}
              menu={menu}
              reload={data.loadSaved}
            />
          </>
        )}
      </div>
      <SidebarFoot ws={ws} loading={data.loading} />
      <div className="side-resize" onMouseDown={size.startResize} onDoubleClick={size.reset} />
      {menu.node}
    </aside>
  );
}
