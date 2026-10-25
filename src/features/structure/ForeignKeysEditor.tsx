// Foreign keys page: existing ones can be dropped; new ones pick both sides' columns.

import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "@/api";
import { FK_ACTIONS } from "@/domain/ddl";
import { type ForeignKeySpec } from "@/types";
import { ColumnChips } from "./ColumnChips";
import {
  type FkRow,
  liveColumnNames,
  newForeignKeyRow,
  patchRow,
  toggleDrop,
} from "./designerModel";
import { DropButton } from "./DropButton";
import { type TableDesignerState } from "./useTableDesigner";

function ActionSelect(props: { value: string; onChange: (v: string) => void }) {
  return (
    <select
      className="select sm"
      value={props.value}
      onChange={(e) => props.onChange(e.target.value)}
    >
      {FK_ACTIONS.map((a) => (
        <option key={a}>{a}</option>
      ))}
    </select>
  );
}

function ExistingForeignKey(props: { row: FkRow; onDrop: () => void }) {
  const f = props.row.orig;
  if (!f) return null;
  return (
    <tr className={props.row.dropped ? "dropped" : ""}>
      <td className="selectable">{f.name}</td>
      <td className="mono">{f.columns.join(", ")}</td>
      <td className="mono">
        {f.refTable} ({f.refColumns.join(", ")})
      </td>
      <td className="small">{f.onUpdate}</td>
      <td className="small">{f.onDelete}</td>
      <td className="center">
        <DropButton dropped={props.row.dropped} what="foreign key" onClick={props.onDrop} />
      </td>
    </tr>
  );
}

/** Columns of the referenced table, loaded when it is picked. */
function useRefColumns(d: TableDesignerState, table: string, fallback: string[]) {
  const [columns, setColumns] = useState<string[]>([]);
  useEffect(() => {
    if (!table) return;
    api
      .listColumns(d.connectionId, d.db, d.schema, table)
      .then((c) => setColumns(c.map((x) => x.name)))
      // A table created in this same change has no columns on the server yet.
      .catch(() => setColumns(fallback));
  }, [table]);
  return columns;
}

function NewForeignKey(props: { d: TableDesignerState; row: FkRow; tables: string[] }) {
  const { d, row: r } = props;
  const own = liveColumnNames(d.cols);
  const refColumns = useRefColumns(d, r.cur.refTable, own);
  const set = (patch: Partial<ForeignKeySpec>) => d.setFks((xs) => patchRow(xs, r.key, patch));
  return (
    <tr className="added">
      <td>
        <input
          className="cell-input"
          placeholder="generated if empty"
          value={r.cur.name}
          onChange={(e) => set({ name: e.target.value })}
        />
      </td>
      <td>
        <ColumnChips names={own} value={r.cur.columns} onChange={(columns) => set({ columns })} />
      </td>
      <td>
        <div className="col tight">
          <select
            className="select sm"
            value={r.cur.refTable}
            onChange={(e) => set({ refTable: e.target.value, refColumns: [] })}
          >
            <option value="">Choose table…</option>
            {props.tables.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
          {r.cur.refTable && (
            <ColumnChips
              names={refColumns}
              value={r.cur.refColumns}
              onChange={(cols) => set({ refColumns: cols })}
            />
          )}
        </div>
      </td>
      <td>
        <ActionSelect value={r.cur.onUpdate} onChange={(onUpdate) => set({ onUpdate })} />
      </td>
      <td>
        <ActionSelect value={r.cur.onDelete} onChange={(onDelete) => set({ onDelete })} />
      </td>
      <td className="center">
        <button
          className="btn ghost icon xs"
          title="Remove"
          onClick={() => d.setFks((xs) => toggleDrop(xs, r.key))}
        >
          <Trash2 size={12} />
        </button>
      </td>
    </tr>
  );
}

export function ForeignKeysEditor({ d }: { d: TableDesignerState }) {
  // A new table can reference itself.
  const typed = d.creating ? d.name.trim() : "";
  const tables = typed ? [...new Set([...d.tables, typed])] : d.tables;
  return (
    <>
      {d.fks.length === 0 ? (
        <div className="faint pad-y">No foreign keys yet.</div>
      ) : (
        <table className="edit-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Columns</th>
              <th>References</th>
              <th>On update</th>
              <th>On delete</th>
              <th className="col-action" />
            </tr>
          </thead>
          <tbody>
            {d.fks.map((r) =>
              r.orig ? (
                <ExistingForeignKey
                  key={r.key}
                  row={r}
                  onDrop={() => d.setFks((xs) => toggleDrop(xs, r.key))}
                />
              ) : (
                <NewForeignKey key={r.key} d={d} row={r} tables={tables} />
              ),
            )}
          </tbody>
        </table>
      )}
      <div className="row designer-add">
        <button className="btn sm" onClick={() => d.setFks((xs) => [...xs, newForeignKeyRow()])}>
          <Plus size={13} /> Add foreign key
        </button>
      </div>
    </>
  );
}
