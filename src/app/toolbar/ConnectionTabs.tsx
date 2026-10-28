// With several connections open, one tab each under the toolbar (TablePlus opens them as
// window tabs), plus the connections page while it is shown.

import { Plus, X } from "lucide-react";
import { connectionColor, isProduction } from "@/domain/connection";
import { cx } from "@/lib/cx";
import { withShortcut } from "@/lib/actions";
import { HOME, useConnection, useStore } from "@/store";
import { type Workspace } from "@/types";

function ConnectionTab({ ws, active }: { ws: Workspace; active: boolean }) {
  const id = ws.connectionId;
  const conn = useConnection(id);
  const { setView, closeWorkspace } = useStore.getState();
  const otherDb = ws.database && ws.database !== ws.info.database;
  return (
    <div
      className={cx("conn-tab", active && "active", isProduction(conn) && "production")}
      role="tab"
      aria-selected={active}
      onMouseDown={(e) => e.button === 0 && setView(id)}
      onAuxClick={(e) => e.button === 1 && closeWorkspace(id)}
      title={`${conn?.name ?? ""} · ${conn?.summary ?? ""}`}
    >
      <button
        className="conn-tab-close"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          void closeWorkspace(id);
        }}
        aria-label="Disconnect"
        title={withShortcut("Disconnect", "disconnect")}
      >
        <X size={11} />
      </button>
      <span className="dot" style={{ background: conn ? connectionColor(conn) : undefined }} />
      <span className="conn-tab-name">{conn?.name ?? id}</span>
      {otherDb && <span className="conn-tab-db">{ws.database}</span>}
      {conn?.options.readOnly && <span className="badge xs read-only">read-only</span>}
    </div>
  );
}

export function ConnectionTabs() {
  const view = useStore((s) => s.view);
  const workspaces = useStore((s) => s.workspaces);
  const list = Object.values(workspaces);
  if (list.length < 2 && view !== HOME) return null;
  return (
    <div className="conn-tabs" role="tablist">
      {list.map((ws) => (
        <ConnectionTab key={ws.connectionId} ws={ws} active={view === ws.connectionId} />
      ))}
      {view === HOME && (
        <div className="conn-tab active" role="tab" aria-selected>
          <span className="conn-tab-name">Connections</span>
        </div>
      )}
      <button
        className="conn-tabs-new"
        onClick={() => useStore.getState().setView(HOME)}
        title={withShortcut("Open a connection", "home")}
        aria-label="Open a connection"
      >
        <Plus size={13} />
      </button>
    </div>
  );
}
