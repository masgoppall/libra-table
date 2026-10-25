// Sidebar of a Redis connection: the numbered databases with their key counts. Empty
// databases are hidden (except the current one) until "Show empty" is on.

import { Database, Eye, EyeOff } from "lucide-react";
import { useEffect, useState } from "react";
import { type ContextMenuApi } from "@/components/ContextMenu";
import { formatCount } from "@/lib/format";
import { showError } from "@/services/errors";
import { store } from "@/store";
import { type Workspace } from "@/types";
import { SideHint, SideSection } from "../sidebar/SideSection";
import { newQueryTab } from "../workspace/tabs";
import { redisApi } from "./api";
import { type DatabaseInfo } from "./types";

/** The key list of a database (one tab per database, reused). */
export const keysTab = (database: string) => ({
  kind: "data" as const,
  title: database,
  schema: "",
  table: "keys",
  database,
});

export function openKeys(ws: Workspace, database: string) {
  const { switchDatabase, addTab } = store.get();
  if (database !== ws.database) switchDatabase(ws.connectionId, database);
  addTab(ws.connectionId, keysTab(database), { reuse: true });
}

function useDatabases(ws: Workspace) {
  const [list, setList] = useState<DatabaseInfo[] | null>(null);
  useEffect(() => {
    redisApi
      .databases(ws.connectionId)
      .then(setList)
      .catch((e) => {
        showError(e);
        setList([]);
      });
  }, [ws.connectionId, ws.refreshKey]);
  return list;
}

export function KeyValueSidebar(props: {
  ws: Workspace;
  query: string;
  collapsed: boolean;
  onToggle: () => void;
  menu: ContextMenuApi;
}) {
  const { ws } = props;
  const list = useDatabases(ws);
  const [showEmpty, setShowEmpty] = useState(false);

  // A fresh workspace opens the key list of its database right away.
  useEffect(() => {
    if (ws.tabs.length === 0) openKeys(ws, ws.database);
  }, []);

  const q = props.query.trim().toLowerCase();
  const shown = (list ?? []).filter(
    (d) => (showEmpty || d.keys > 0 || d.name === ws.database) && d.name.includes(q),
  );
  const menuFor = (d: DatabaseInfo) => [
    { label: "Open keys", onClick: () => openKeys(ws, d.name) },
    {
      label: "New query in this database",
      onClick: () => store.get().addTab(ws.connectionId, newQueryTab("", { database: d.name })),
    },
  ];
  const toggle = (
    <button
      className="icon-btn"
      title={showEmpty ? "Hide empty databases" : "Show empty databases"}
      onClick={() => setShowEmpty(!showEmpty)}
    >
      {showEmpty ? <EyeOff size={13} /> : <Eye size={13} />}
    </button>
  );
  return (
    <SideSection
      label="Databases"
      count={shown.length}
      collapsed={props.collapsed}
      onToggle={props.onToggle}
      action={toggle}
    >
      {list && shown.length === 0 && <SideHint>No databases match.</SideHint>}
      {shown.map((d) => (
        <div
          key={d.name}
          className={`side-item db${d.name === ws.database ? " current" : ""}`}
          onClick={() => openKeys(ws, d.name)}
          onContextMenu={(e) => props.menu.open(e, menuFor(d))}
          title={`${d.name}: ${formatCount(d.keys)} keys, ${formatCount(d.expires)} with a TTL`}
        >
          <Database size={13} className="side-icon db" />
          <span className="label">{d.name}</span>
          <span className="side-count">{formatCount(d.keys)}</span>
        </div>
      ))}
    </SideSection>
  );
}
