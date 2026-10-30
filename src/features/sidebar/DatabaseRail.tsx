// The databases opened in this connection, in a column left of the sidebar (TablePlus'
// database stack): a click shows one in the sidebar, ⌘K or "+" opens another, and
// "Close database" takes one off with its tabs.

import { Plus } from "lucide-react";
import { useContextMenu, type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { cx } from "@/lib/cx";
import { copyText } from "@/lib/clipboard";
import { dispatch, withShortcut } from "@/lib/actions";
import { confirmDiscard } from "@/services/confirm";
import { store } from "@/store";
import { openDatabases } from "@/store/workspaceModel";
import { type Workspace } from "@/types";

/** An outlined database cylinder. */
function DatabaseIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 26 26" aria-hidden>
      <g fill="none" stroke="currentColor" strokeWidth="1.4">
        <ellipse cx="13" cy="6" rx="8.5" ry="3" />
        <path d="M4.5 6v14c0 1.7 3.8 3 8.5 3s8.5-1.3 8.5-3V6" />
        <path d="M4.5 10.7c0 1.7 3.8 3 8.5 3s8.5-1.3 8.5-3" />
        <path d="M4.5 15.4c0 1.7 3.8 3 8.5 3s8.5-1.3 8.5-3" />
      </g>
    </svg>
  );
}

async function closeDatabase(ws: Workspace, database: string) {
  const { pendingEdits } = store.get();
  const dirty = ws.tabs
    .filter((t) => (t.database ?? ws.info.database) === database)
    .reduce((n, t) => n + (pendingEdits[t.id] ?? 0), 0);
  const wording = { doing: `Closing ${database}`, verb: "close", kind: "unsaved" };
  if (await confirmDiscard(dirty, wording)) store.get().closeDatabase(ws.connectionId, database);
}

function railMenu(ws: Workspace, database: string): MenuItem[] {
  return [
    {
      label: "Show in sidebar",
      disabled: database === ws.database,
      onClick: () => store.get().switchDatabase(ws.connectionId, database),
    },
    { label: "Open database…", hint: "⌘K", onClick: () => dispatch("switch-database") },
    { label: "Copy name", onClick: () => void copyText(database) },
    SEPARATOR,
    { label: "Close database", onClick: () => void closeDatabase(ws, database) },
  ];
}

export function DatabaseRail({ ws }: { ws: Workspace }) {
  const menu = useContextMenu();
  const databases = openDatabases(ws);
  if (!ws.caps.switchDatabase || databases.length < 2) return null;
  return (
    <nav className={cx("db-rail", ws.sidebarHidden && "hidden")} aria-label="Open databases">
      <div className="db-rail-list" role="tablist">
        {databases.map((d) => (
          <button
            key={d}
            role="tab"
            aria-selected={d === ws.database}
            className={cx("db-rail-item", d === ws.database && "active")}
            title={d}
            onClick={() => store.get().switchDatabase(ws.connectionId, d)}
            onContextMenu={(e) => menu.open(e, railMenu(ws, d))}
          >
            <DatabaseIcon />
            <span className="db-rail-name">{d}</span>
          </button>
        ))}
      </div>
      <button
        className="db-rail-add"
        title={withShortcut("Open database", "switch-database")}
        aria-label="Open database"
        onClick={() => dispatch("switch-database")}
      >
        <Plus size={14} />
      </button>
      {menu.node}
    </nav>
  );
}
