// ⌘K "Open database": TablePlus' sheet with a search field, the server's databases and
// Cancel / New… / Open. Right-click a database for its other actions (backup, drop…).

import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "@/api";
import { type MenuItem, useContextMenu } from "@/components/ContextMenu";
import { Highlight } from "@/components/Highlight";
import { Sheet, SheetSearch } from "@/components/Sheet";
import { Spinner } from "@/components/Spinner";
import { LAYER, useActions } from "@/lib/actions";
import { search } from "@/lib/fuzzy";
import { showError } from "@/services/errors";
import { store } from "@/store";
import { type Workspace } from "@/types";
import { createDatabaseFlow } from "../schema/flows";
import { databaseMenu } from "../sidebar/sidebarMenus";

/** The stacked-disks database glyph of the list. */
function DatabaseGlyph() {
  return (
    <svg className="dbs-icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden>
      <g fill="currentColor">
        <ellipse cx="8" cy="3.2" rx="7" ry="2.6" />
        <path d="M1 5.1c.7 1.4 3.6 2.4 7 2.4s6.3-1 7-2.4v2.5c0 1.4-3.1 2.6-7 2.6s-7-1.2-7-2.6z" />
        <path d="M1 9.4c.7 1.4 3.6 2.4 7 2.4s6.3-1 7-2.4v2.9c0 1.5-3.1 2.7-7 2.7s-7-1.2-7-2.7z" />
      </g>
      <g fill="none" stroke="var(--sh-bg)" strokeWidth="0.7" opacity="0.55">
        <path d="M1 5.1c.7 1.4 3.6 2.4 7 2.4s6.3-1 7-2.4M1 9.4c.7 1.4 3.6 2.4 7 2.4s6.3-1 7-2.4" />
      </g>
    </svg>
  );
}

function useDatabases(ws: Workspace) {
  const [list, setList] = useState<string[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    api
      .listDatabases(ws.connectionId)
      .then((dbs) => !cancelled && setList(dbs))
      .catch((e) => {
        showError(e);
        if (!cancelled) setList([]);
      });
    return () => {
      cancelled = true;
    };
  }, [ws.connectionId]);
  return list;
}

export function DatabaseSheet({ ws }: { ws: Workspace }) {
  const id = ws.connectionId;
  const { closePalette, openPalette } = store.get();
  const list = useDatabases(ws);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const menu = useContextMenu();
  const matches = useMemo(() => search(list ?? [], query, "fuzzy", (d) => d), [list, query]);
  const names = matches.map((m) => m.item);
  // Keep a visible selection: the current database first, then the best match.
  const current =
    selected && names.includes(selected)
      ? selected
      : !query && names.includes(ws.database)
        ? ws.database
        : (names[0] ?? null);

  useActions(LAYER.dialog, true, {
    "switch-database": closePalette,
    "open-anything": () => openPalette("anything", id),
  });
  useEffect(() => {
    listRef.current?.querySelector(".dbs-row.selected")?.scrollIntoView({ block: "nearest" });
  }, [current]);

  const open = (name: string | null) => {
    if (!name) return;
    closePalette();
    if (name !== ws.database) store.get().switchDatabase(id, name);
  };
  const newDatabase = () => {
    closePalette();
    setTimeout(() => createDatabaseFlow(ws), 0);
  };
  /** Menu actions run once the sheet is gone, so their own dialogs are on top. */
  const rowMenu = (name: string): MenuItem[] =>
    databaseMenu(ws, name).map((item) => {
      const run = item.onClick;
      if (!run) return item;
      return {
        ...item,
        onClick: () => {
          closePalette();
          setTimeout(run, 0);
        },
      };
    });

  const onKeyDown = (e: React.KeyboardEvent) => {
    const at = current ? names.indexOf(current) : -1;
    const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (step) {
      e.preventDefault();
      const next = names[Math.max(0, Math.min(names.length - 1, at + step))];
      if (next) setSelected(next);
    } else if (e.key === "Enter") {
      e.preventDefault();
      open(current);
    }
  };

  return (
    <Sheet
      label="Open database"
      className="dbs-sheet"
      onClose={closePalette}
      footer={
        <>
          <button className="sheet-btn" onClick={closePalette}>
            Cancel
          </button>
          <span className="spacer" />
          <button className="sheet-btn" onClick={newDatabase} disabled={!ws.caps.createDatabase}>
            New...
          </button>
          <button className="sheet-btn primary" onClick={() => open(current)} disabled={!current}>
            Open
          </button>
        </>
      }
    >
      <div className="dbs-title">Open database</div>
      <div className="dbs-body" onKeyDown={onKeyDown}>
        <SheetSearch value={query} onChange={setQuery} placeholder="Search for database..." />
        <div className="dbs-list" ref={listRef} role="listbox" aria-label="Databases">
          {list === null && (
            <div className="dbs-state">
              <Spinner size={18} />
            </div>
          )}
          {list !== null && names.length === 0 && (
            <div className="dbs-state">
              {query ? `No database matches “${query}”.` : "This server has no databases."}
            </div>
          )}
          {matches.map(({ item: d, match }) => (
            <div
              key={d}
              role="option"
              aria-selected={d === current}
              className={`dbs-row${d === current ? " selected" : ""}`}
              onClick={() => setSelected(d)}
              onDoubleClick={() => open(d)}
              onContextMenu={(e) => {
                setSelected(d);
                menu.open(e, rowMenu(d));
              }}
            >
              <DatabaseGlyph />
              <span className="dbs-name">
                <Highlight text={d} indexes={match.indexes} />
              </span>
              {d === ws.database && <span className="dbs-current">current</span>}
            </div>
          ))}
        </div>
      </div>
      {menu.node}
    </Sheet>
  );
}
