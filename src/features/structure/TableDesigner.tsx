// Table structure: create a table, or change an existing one (columns, primary key,
// indexes, foreign keys). Changes are collected, previewed as SQL and applied in one go.
// Views show their columns and definition read-only.

import { ErrorCallout, PaddedError } from "@/components/ErrorCallout";
import { Spinner } from "@/components/Spinner";
import { LAYER, useActions } from "@/lib/actions";
import { store, useIsTabVisible } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { tableTab } from "../workspace/tabs";
import { ColumnsEditor } from "./ColumnsEditor";
import { DdlBlock } from "./DdlBlock";
import { DesignerBar } from "./DesignerBar";
import { DesignerToolbar } from "./DesignerToolbar";
import { ForeignKeysEditor } from "./ForeignKeysEditor";
import { IndexesEditor } from "./IndexesEditor";
import { TypeSuggestions } from "./TypeSuggestions";
import { type TableDesignerState, useTableDesigner } from "./useTableDesigner";
import { ViewStructure } from "./ViewStructure";

function SectionBody({ d }: { d: TableDesignerState }) {
  switch (d.section) {
    case "columns":
      return <ColumnsEditor d={d} />;
    case "indexes":
      return <IndexesEditor d={d} />;
    case "foreign":
      return <ForeignKeysEditor d={d} />;
    case "ddl":
      return d.structure ? <DdlBlock ddl={d.structure.ddl} /> : null;
  }
}

export function TableDesigner({ ws, tab }: { ws: Workspace; tab: WorkspaceTab }) {
  const id = ws.connectionId;
  const d = useTableDesigner(ws, tab);
  const visible = useIsTabVisible(id, tab.id);
  const where = `${tab.database ?? ws.database}${d.driver === "postgres" ? `.${d.schema}` : ""}`;

  const openData = () => {
    if (!tab.table) return;
    store.get().addTab(id, tableTab("data", tab.table, tab), { reuse: true });
  };

  useActions(LAYER.tab, visible, {
    save: () => (d.isView ? false : void d.apply()),
    refresh: () => (d.creating ? false : void d.reload()),
    "add-row": () => (d.isView ? false : d.addColumn()),
    "open-structure": openData,
  });

  const toolbar = <DesignerToolbar d={d} where={where} onOpenData={openData} />;

  if (!d.creating && !d.structure) {
    return (
      <div className="tab-body">
        {toolbar}
        {d.error ? (
          <PaddedError error={d.error} />
        ) : (
          <div className="grid-empty">
            <Spinner /> Loading structure…
          </div>
        )}
      </div>
    );
  }

  if ((d.isView || ws.caps.structure === "view") && d.structure) {
    return (
      <div className="tab-body">
        {toolbar}
        <ViewStructure structure={d.structure} />
      </div>
    );
  }

  return (
    <div className="tab-body">
      {toolbar}
      <div className="designer">
        <ErrorCallout error={d.error} />
        <SectionBody d={d} />
      </div>
      <DesignerBar d={d} />
      <TypeSuggestions driver={d.driver} />
    </div>
  );
}
