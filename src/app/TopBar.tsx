// Title bar: the connections page, one tab per open connection, search and settings.

import { LayoutGrid, Search, Settings, X } from "lucide-react";
import { connectionColor, isProduction } from "@/domain/connection";
import { dispatch, shortcutFor, withShortcut } from "@/lib/actions";
import { HOME, useConnection, useStore } from "@/store";
import { type Workspace } from "@/types";

function ConnectionTab({ ws, active }: { ws: Workspace; active: boolean }) {
  const id = ws.connectionId;
  const conn = useConnection(id);
  const { setView, closeWorkspace } = useStore.getState();
  const prod = isProduction(conn);
  const otherDb = ws.database && ws.database !== ws.info.database;
  return (
    <div
      className={`toptab${active ? " active" : ""}${prod ? " production" : ""}`}
      onMouseDown={(e) => e.button === 0 && setView(id)}
      onAuxClick={(e) => e.button === 1 && closeWorkspace(id)}
      title={`${conn?.name ?? ""} · ${conn?.summary ?? ""}`}
    >
      <span className="dot" style={{ background: conn ? connectionColor(conn) : undefined }} />
      <span className="name">{conn?.name ?? id}</span>
      {otherDb && <span className="toptab-db">{ws.database}</span>}
      {prod && <span className="badge production xs">prod</span>}
      {conn?.options.readOnly && <span className="badge xs read-only">read-only</span>}
      <button
        className="toptab-close"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          closeWorkspace(id);
        }}
        aria-label="Disconnect"
        title={withShortcut("Disconnect", "disconnect")}
      >
        <X size={12} />
      </button>
    </div>
  );
}

export function TopBar() {
  const view = useStore((s) => s.view);
  const workspaces = useStore((s) => s.workspaces);
  const setView = useStore((s) => s.setView);
  const setSettingsOpen = useStore((s) => s.setSettingsOpen);
  const inWorkspace = view !== HOME && !!workspaces[view];
  return (
    <div className="topbar" data-tauri-drag-region>
      <div className="traffic-space" data-tauri-drag-region />
      <button
        className={`toptab home-tab${view === HOME ? " active" : ""}`}
        onClick={() => setView(HOME)}
        title={withShortcut("Connections", "home")}
      >
        <LayoutGrid size={14} />
        <span className="name">Connections</span>
      </button>
      <div className="toptabs" data-tauri-drag-region>
        {Object.values(workspaces).map((ws) => (
          <ConnectionTab key={ws.connectionId} ws={ws} active={view === ws.connectionId} />
        ))}
      </div>
      <div className="topbar-fill" data-tauri-drag-region />
      {inWorkspace && (
        <button
          className="topbar-search"
          onClick={() => dispatch("open-anything")}
          title={withShortcut("Open a table, view or query", "open-anything")}
        >
          <Search size={13} />
          <span>Open table…</span>
          <kbd>{shortcutFor("open-anything")}</kbd>
        </button>
      )}
      <button
        className="topbar-btn"
        onClick={() => setSettingsOpen(true)}
        title={withShortcut("Settings", "settings")}
        aria-label="Settings"
      >
        <Settings size={14} />
      </button>
    </div>
  );
}
