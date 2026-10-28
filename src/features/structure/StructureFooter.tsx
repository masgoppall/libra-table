// The bar under the structure grid: Data | Structure | Index, then + Column (+ Index),
// Triggers (the table's triggers, to open one) and DDL.

import { api } from "@/api";
import { type ContextMenuApi, type MenuItem } from "@/components/ContextMenu";
import { showError } from "@/services/errors";
import { type Workspace, type WorkspaceTab } from "@/types";
import { openDefinition } from "../objects/objectFlows";
import { switchTableView, type TableSegment, TableViewSwitch } from "../workspace/TableViewSwitch";
import { type TableDesignerState } from "./useTableDesigner";

export function StructureFooter(p: {
  ws: Workspace;
  tab: WorkspaceTab;
  d: TableDesignerState;
  menu: ContextMenuApi;
  readOnly: string | null;
  onAdd: () => void;
  onDdl: () => void;
}) {
  const { d, ws } = p;
  const segment: TableSegment = d.section === "indexes" ? "index" : "structure";

  const pick = (s: TableSegment) => {
    if (s === "data") void switchTableView(ws, p.tab, "data");
    else d.setSection(s === "index" ? "indexes" : "columns");
  };

  const triggers = async (el: HTMLElement) => {
    try {
      const all = await api.listSchemaObjects(d.connectionId, d.db, d.schema);
      const mine = all.filter((o) => o.kind === "trigger" && o.table === d.table);
      const items: MenuItem[] = mine.length
        ? mine.map((o) => ({
            label: o.name,
            hint: o.detail ?? undefined,
            onClick: () => openDefinition(ws, o, d.schema),
          }))
        : [{ label: "No triggers on this table", disabled: true }];
      p.menu.openAbove(el, items);
    } catch (e) {
      showError(e);
    }
  };

  const adding = d.section === "indexes" ? "Index" : "Column";
  return (
    <div className="st-foot">
      <TableViewSwitch
        segments={["data", "structure", "index"]}
        current={segment}
        onPick={pick}
        disabled={d.creating ? ["data"] : undefined}
      />
      <button
        className="st-btn"
        aria-label={`Add ${adding.toLowerCase()}`}
        disabled={!!p.readOnly}
        onClick={p.onAdd}
      >
        <span className="st-plus" aria-hidden>
          +
        </span>{" "}
        {adding}
      </button>
      <button
        className="st-btn"
        disabled={d.creating || !ws.caps.schemaObjects}
        onClick={(e) => void triggers(e.currentTarget)}
      >
        Triggers
      </button>
      <button className="st-btn" disabled={d.creating || !d.structure} onClick={p.onDdl}>
        DDL
      </button>
    </div>
  );
}
