// Database switch, schema picker and search at the top of the sidebar.

import { ChevronDown, ChevronsUpDown, Database, Search } from "lucide-react";
import { type ContextMenuApi } from "@/components/ContextMenu";
import { type MatchMode } from "@/lib/fuzzy";
import { withShortcut } from "@/lib/actions";
import { store } from "@/store";
import { type Workspace } from "@/types";
import { schemaMenu } from "./sidebarMenus";
import { hasSchemaPicker } from "./useSidebarData";

function DatabaseSwitch({ ws }: { ws: Workspace }) {
  return (
    <button
      className="db-switch"
      onClick={() => store.get().openPalette("database", ws.connectionId)}
      title={withShortcut("Switch database", "switch-database")}
    >
      <Database size={14} className="db-switch-icon" />
      <span className="db-switch-name">{ws.database || "Choose a database"}</span>
      <ChevronsUpDown size={13} className="faint" />
    </button>
  );
}

function SchemaPicker(props: { ws: Workspace; schemas: string[]; menu: ContextMenuApi }) {
  const { ws, schemas } = props;
  const isSqlite = ws.info.dialect === "sqlite";
  const set = (schema: string) => store.get().updateWorkspace(ws.connectionId, { schema });
  return (
    <div className="row tight">
      <select
        className="select sm grow"
        value={ws.schema}
        onChange={(e) => set(e.target.value)}
        title={isSqlite ? "Attached database" : "Schema"}
      >
        {!schemas.includes(ws.schema) && <option value={ws.schema}>{ws.schema}</option>}
        {schemas.map((s) => (
          <option key={s} value={s}>
            {s}
          </option>
        ))}
      </select>
      {ws.info.dialect === "postgres" && (
        <button
          className="btn sm icon"
          title="Schema actions"
          onClick={(e) => props.menu.openAt(e.currentTarget, schemaMenu(ws))}
        >
          <ChevronDown size={13} />
        </button>
      )}
    </div>
  );
}

export interface SidebarSearch {
  query: string;
  setQuery: (q: string) => void;
  mode: MatchMode;
  setMode: (m: MatchMode) => void;
}

function SearchBox({ search }: { search: SidebarSearch }) {
  const fuzzy = search.mode === "fuzzy";
  return (
    <div className="side-search">
      <Search size={13} className="faint" />
      <input
        placeholder="Search"
        value={search.query}
        onChange={(e) => search.setQuery(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && search.setQuery("")}
      />
      <button
        className="side-mode"
        onClick={() => search.setMode(fuzzy ? "contains" : "fuzzy")}
        title={fuzzy ? "Fuzzy match (click for contains)" : "Contains (click for fuzzy)"}
      >
        {fuzzy ? "~" : "ab"}
      </button>
    </div>
  );
}

export function SidebarTop(props: {
  ws: Workspace;
  schemas: string[];
  menu: ContextMenuApi;
  search: SidebarSearch;
}) {
  const { ws } = props;
  return (
    <div className="sidebar-top">
      {ws.caps.switchDatabase && <DatabaseSwitch ws={ws} />}
      {hasSchemaPicker(ws) && props.schemas.length > 0 && (
        <SchemaPicker ws={ws} schemas={props.schemas} menu={props.menu} />
      )}
      <SearchBox search={props.search} />
    </div>
  );
}
