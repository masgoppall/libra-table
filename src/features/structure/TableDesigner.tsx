// A table's structure as TablePlus shows it: Name / Primary / search above, the column
// grid (or the indexes), and Data | Structure | Index, + Column, Triggers and DDL below.
// Edits are collected and applied with ⌘S (or the toolbar's commit), after the SQL is
// previewed. Views and engines without DDL show the same grid read-only.

import { useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { PaddedError } from "@/components/ErrorCallout";
import { LoadingState } from "@/components/StateView";
import { LAYER, useActions } from "@/lib/actions";
import { useIsTabVisible } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { switchTableView } from "../workspace/TableViewSwitch";
import { ColumnGrid } from "./ColumnGrid";
import { DdlSheet } from "./DdlSheet";
import { type ColRow } from "./designerModel";
import { ForeignKeyPopover } from "./ForeignKeyPopover";
import { IndexGrid } from "./IndexGrid";
import { StructureFooter } from "./StructureFooter";
import { StructureHeader } from "./StructureHeader";
import { useTableDesigner } from "./useTableDesigner";

const VIEW_READ_ONLY =
  "The structure of a view can't be edited here. Change its definition (DDL) instead.";
const ENGINE_READ_ONLY = "This database's structure can't be changed from LibraTable.";

export function TableDesigner({ ws, tab }: { ws: Workspace; tab: WorkspaceTab }) {
  const id = ws.connectionId;
  const d = useTableDesigner(ws, tab);
  const visible = useIsTabVisible(id, tab.id);
  const menu = useContextMenu();
  const [search, setSearch] = useState("");
  const [focus, setFocus] = useState<string | null>(null);
  const [fk, setFk] = useState<{ row: ColRow; anchor: HTMLElement } | null>(null);
  const [ddl, setDdl] = useState(false);
  let readOnly: string | null = null;
  if (d.isView) readOnly = VIEW_READ_ONLY;
  else if (ws.caps.structure === "view") readOnly = ENGINE_READ_ONLY;

  const openData = () => void switchTableView(ws, tab, "data");
  const add = () => {
    if (readOnly) return;
    setSearch("");
    setFocus(d.section === "indexes" ? d.addIndex() : d.addColumn());
  };

  useActions(LAYER.tab, visible, {
    save: () => (readOnly ? false : void d.apply()),
    discard: () => (readOnly || (!d.creating && !d.pending) ? false : d.discard()),
    refresh: () => (d.creating ? false : void d.reload()),
    "add-row": () => (readOnly ? false : add()),
    "open-structure": openData,
  });

  const loaded = d.creating || !!d.structure;
  let body;
  if (!loaded) {
    body = d.error ? <PaddedError error={d.error} /> : <LoadingState label="Loading structure…" />;
  } else if (d.section === "indexes") {
    body = <IndexGrid d={d} menu={menu} readOnly={readOnly} focus={focus} />;
  } else {
    body = (
      <ColumnGrid
        d={d}
        search={search}
        menu={menu}
        readOnly={readOnly}
        focus={focus}
        onForeignKey={(row, anchor) => setFk({ row, anchor })}
      />
    );
  }

  return (
    <div className="tab-body structure-tab">
      <StructureHeader d={d} menu={menu} readOnly={readOnly} search={search} onSearch={setSearch} />
      {loaded && d.error && <PaddedError error={d.error} />}
      <div className="st-body">{body}</div>
      <StructureFooter
        ws={ws}
        tab={tab}
        d={d}
        menu={menu}
        readOnly={readOnly}
        onAdd={add}
        onDdl={() => setDdl(true)}
      />
      {menu.node}
      {fk && (
        <ForeignKeyPopover d={d} row={fk.row} anchor={fk.anchor} onClose={() => setFk(null)} />
      )}
      {ddl && d.structure && (
        <DdlSheet table={d.table} ddl={d.structure.ddl} onClose={() => setDdl(false)} />
      )}
    </div>
  );
}
