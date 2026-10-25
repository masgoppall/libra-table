// Tabs stay mounted so editors and grids keep their state when switching.

import { lazy, Suspense } from "react";
import { type Workspace, type WorkspaceTab } from "@/types";
import { DataView } from "../data/DataView";
import { HistoryView } from "../history/HistoryView";
import { KeysView } from "../keyvalue/KeysView";
import { DefinitionView } from "../objects/DefinitionView";
import { ProcessListView } from "../server/processes/ProcessListView";
import { UsersView } from "../server/users/UsersView";
import { TableDesigner } from "../structure/TableDesigner";

// The SQL editor (CodeMirror) is the largest dependency; load it with the first query tab.
const QueryView = lazy(() => import("../query/QueryView").then((m) => ({ default: m.QueryView })));
const DiagramView = lazy(() =>
  import("../diagram/DiagramView").then((m) => ({ default: m.DiagramView })),
);
const MetricsView = lazy(() =>
  import("../metrics/MetricsView").then((m) => ({ default: m.MetricsView })),
);

const loading = <div className="message faint">Loading…</div>;

function TabContent({ ws, tab, visible }: { ws: Workspace; tab: WorkspaceTab; visible: boolean }) {
  switch (tab.kind) {
    case "data":
      if (ws.caps.kind === "key_value") return <KeysView ws={ws} tab={tab} />;
      return tab.schema !== undefined && tab.table ? <DataView ws={ws} tab={tab} /> : null;
    case "structure":
    case "new-table":
      return <TableDesigner ws={ws} tab={tab} />;
    case "query":
      return (
        <Suspense fallback={<div className="message faint">Loading editor…</div>}>
          <QueryView ws={ws} tab={tab} />
        </Suspense>
      );
    case "history":
      return <HistoryView ws={ws} visible={visible} />;
    case "definition":
      return tab.object ? <DefinitionView ws={ws} tab={tab} object={tab.object} /> : null;
    case "processes":
      return <ProcessListView ws={ws} visible={visible} />;
    case "users":
      return <UsersView ws={ws} tab={tab} visible={visible} />;
    case "diagram":
      return (
        <Suspense fallback={loading}>
          <DiagramView ws={ws} tab={tab} />
        </Suspense>
      );
    case "metrics":
      return (
        <Suspense fallback={loading}>
          <MetricsView ws={ws} tab={tab} visible={visible} />
        </Suspense>
      );
  }
}

export function TabPanes({ ws, visible }: { ws: Workspace; visible: boolean }) {
  return (
    <>
      {ws.tabs.map((t) => {
        const active = t.id === ws.activeTabId;
        return (
          <div key={t.id} className="tab-pane" style={{ display: active ? "flex" : "none" }}>
            <TabContent ws={ws} tab={t} visible={visible && active} />
          </div>
        );
      })}
    </>
  );
}
