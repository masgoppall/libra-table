// Tabs of a workspace: click, middle-click to close, drag to reorder, right-click menu.

import { Plus, X } from "lucide-react";
import { useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { withShortcut } from "@/lib/actions";
import { store } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { tabMenu } from "./tabMenu";
import { newQueryTab, TAB_ICONS, tabTooltip } from "./tabs";

const DRAG_TYPE = "text/x-tab";

interface TabProps {
  ws: Workspace;
  tab: WorkspaceTab;
  index: number;
  showDatabase: boolean;
  dragging: string | null;
  setDragging: (id: string | null) => void;
  onMenu: (e: React.MouseEvent) => void;
}

function Tab({ ws, tab: t, index, showDatabase, dragging, setDragging, onMenu }: TabProps) {
  const id = ws.connectionId;
  const { setActiveTab, closeTab, moveTab } = store.get();
  const Icon = TAB_ICONS[t.kind];
  const classes = [
    "tab",
    `kind-${t.kind}`,
    t.id === ws.activeTabId && "active",
    dragging === t.id && "dragging",
  ];
  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      onMouseDown={(e) => e.button === 0 && setActiveTab(id, t.id)}
      onAuxClick={(e) => e.button === 1 && closeTab(id, t.id)}
      onContextMenu={onMenu}
      title={tabTooltip(ws, t)}
      draggable
      onDragStart={(e) => {
        setDragging(t.id);
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData(DRAG_TYPE, t.id);
      }}
      onDragEnd={() => setDragging(null)}
      onDragOver={(e) => dragging && e.preventDefault()}
      onDrop={(e) => {
        const from = e.dataTransfer.getData(DRAG_TYPE);
        if (from && from !== t.id) moveTab(id, from, index);
        setDragging(null);
      }}
    >
      <Icon size={13} className="tab-icon" />
      <span className="tname">{t.title}</span>
      {showDatabase && <span className="tab-db">{t.database ?? ws.database}</span>}
      <button
        className="tab-close"
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          closeTab(id, t.id);
        }}
        aria-label="Close tab"
      >
        <X size={12} />
      </button>
    </div>
  );
}

export function TabBar({ ws }: { ws: Workspace }) {
  const [dragging, setDragging] = useState<string | null>(null);
  const menu = useContextMenu();
  const newQuery = () => store.get().addTab(ws.connectionId, newQueryTab());
  // Once tabs from two databases are open, every tab says which one it is in.
  const showDatabase = new Set(ws.tabs.map((t) => t.database ?? ws.database)).size > 1;
  return (
    <div className="tabbar" onDoubleClick={(e) => e.target === e.currentTarget && newQuery()}>
      {ws.tabs.map((t, i) => (
        <Tab
          key={t.id}
          ws={ws}
          tab={t}
          index={i}
          showDatabase={showDatabase}
          dragging={dragging}
          setDragging={setDragging}
          onMenu={(e) => menu.open(e, tabMenu(ws, t))}
        />
      ))}
      <button className="tab-new" title={withShortcut("New query", "new-query")} onClick={newQuery}>
        <Plus size={14} />
      </button>
      {menu.node}
    </div>
  );
}
