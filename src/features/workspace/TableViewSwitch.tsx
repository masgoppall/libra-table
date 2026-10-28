// The segmented control at the bottom left of a table tab: Data | Structure (| Index).
// Like TablePlus, the tab itself switches between its rows and its structure.

import { confirmDiscard } from "@/services/confirm";
import { store } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";

export type TableSegment = "data" | "structure" | "index";

const LABELS: Record<TableSegment, string> = {
  data: "Data",
  structure: "Structure",
  index: "Index",
};

/** Shows the table's rows or its structure in this tab, asking before dropping edits. */
export async function switchTableView(
  ws: Workspace,
  tab: WorkspaceTab,
  kind: "data" | "structure",
) {
  if (tab.kind === kind || !tab.table) return;
  const dirty = store.get().pendingEdits[tab.id] ?? 0;
  const wording = { doing: "Switching views", verb: "switch", kind: "unsaved" };
  if (!(await confirmDiscard(dirty, wording))) return;
  store.get().updateTab(ws.connectionId, tab.id, { kind });
}

export function TableViewSwitch(p: {
  segments: TableSegment[];
  current: TableSegment;
  onPick: (s: TableSegment) => void;
  disabled?: TableSegment[];
}) {
  return (
    <div className="st-seg" role="tablist" aria-label="View">
      {p.segments.map((s) => (
        <button
          key={s}
          role="tab"
          aria-selected={p.current === s}
          className={p.current === s ? "on" : ""}
          disabled={p.disabled?.includes(s)}
          onClick={() => p.current !== s && p.onPick(s)}
        >
          {LABELS[s]}
        </button>
      ))}
    </div>
  );
}
