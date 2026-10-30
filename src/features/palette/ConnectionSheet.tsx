// ⇧⌘K "Open connection" from inside a connection (TablePlus' connection picker): the saved
// connections in their groups, a search field, and Cancel / New… / Open. The connection
// opens next to the current one (a connection tab); one already open comes to the front.

import { useMemo, useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { Sheet, SheetSearch } from "@/components/Sheet";
import { LAYER, useActions } from "@/lib/actions";
import { openConnection, openInNewWindow } from "@/services/connect";
import { alertError } from "@/services/errors";
import { store, useStore } from "@/store";
import { type ConnectionView } from "@/types";
import { toggleGroup } from "../connections/connectionActions";
import { ConnectionTree } from "../connections/welcome/ConnectionTree";
import { buildTree, groupKey, type TreeRow } from "../connections/welcome/tree";

const rowKey = (r: TreeRow) => (r.kind === "group" ? groupKey(r.group.id) : r.conn.id);

export function ConnectionSheet() {
  const { closePalette, requestNewConnection } = store.get();
  const connections = useStore((s) => s.connections);
  const groups = useStore((s) => s.groups);
  const workspaces = useStore((s) => s.workspaces);
  const openIds = Object.keys(workspaces);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [connecting, setConnecting] = useState<string | null>(null);
  const menu = useContextMenu();
  const rows = useMemo(
    () => buildTree(connections, groups, query, "manual"),
    [connections, groups, query],
  );
  const keys = rows.map(rowKey);
  // Keep a visible selection: the pick, else the first connection.
  const current =
    selected && keys.includes(selected)
      ? selected
      : (rows.find((r) => r.kind === "conn")?.conn.id ?? keys[0] ?? null);
  const currentRow = rows.find((r) => rowKey(r) === current);

  useActions(LAYER.dialog, true, { "open-connection": closePalette });

  const open = async (c: ConnectionView) => {
    setConnecting(c.id);
    try {
      if (await openConnection(c)) closePalette();
    } catch (e) {
      void alertError(`Couldn't connect to ${c.name}`, e, "connection-failed");
    } finally {
      setConnecting(null);
    }
  };
  const activate = (row: TreeRow | undefined) => {
    if (row?.kind === "conn") void open(row.conn);
    else if (row?.kind === "group") void toggleGroup(row.group);
  };
  const newConnection = () => {
    closePalette();
    requestNewConnection();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const step = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    const at = current ? keys.indexOf(current) : -1;
    if (step) {
      e.preventDefault();
      const next = keys[Math.max(0, Math.min(keys.length - 1, at + step))];
      if (next) setSelected(next);
    } else if (e.key === "Enter") {
      e.preventDefault();
      activate(currentRow);
    }
  };

  return (
    <Sheet
      label="Open connection"
      className="dbs-sheet conn-sheet w-tokens"
      onClose={closePalette}
      footer={
        <>
          <button className="sheet-btn" onClick={closePalette}>
            Cancel
          </button>
          <span className="spacer" />
          <button className="sheet-btn" onClick={newConnection}>
            New...
          </button>
          <button
            className="sheet-btn primary"
            onClick={() => activate(currentRow)}
            disabled={currentRow?.kind !== "conn" || connecting !== null}
          >
            Open
          </button>
        </>
      }
    >
      <div className="dbs-title">Open Connection</div>
      <div className="dbs-body" onKeyDown={onKeyDown}>
        <SheetSearch value={query} onChange={setQuery} placeholder="Search for connection..." />
        <div className="dbs-list conn-sheet-list">
          {rows.length === 0 ? (
            <div className="dbs-state">
              {query ? `No connection matches “${query}”.` : "No saved connections yet."}
            </div>
          ) : (
            <ConnectionTree
              rows={rows}
              openIds={openIds}
              selected={current}
              connecting={connecting}
              menu={menu}
              onSelect={setSelected}
              onConnect={(c) => void open(c)}
              connectionMenu={(c) => [
                { label: "Open", onClick: () => void open(c) },
                {
                  label: "Open in New Window",
                  onClick: () => {
                    closePalette();
                    void openInNewWindow(c.id);
                  },
                },
              ]}
              groupMenu={(g) => [
                { label: g.collapsed ? "Expand" : "Collapse", onClick: () => void toggleGroup(g) },
              ]}
            />
          )}
        </div>
      </div>
      {menu.node}
    </Sheet>
  );
}
