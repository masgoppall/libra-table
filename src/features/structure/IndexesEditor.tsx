// Indexes page: existing ones can be dropped, new ones get a name, columns and UNIQUE.

import { KeyRound, Plus } from "lucide-react";
import { type IndexSpec } from "@/types";
import { ColumnChips } from "./ColumnChips";
import { type IdxRow, liveColumnNames, newIndexRow, patchRow, toggleDrop } from "./designerModel";
import { DropButton } from "./DropButton";
import { type TableDesignerState } from "./useTableDesigner";

function IndexRowEditor({ d, row: r }: { d: TableDesignerState; row: IdxRow }) {
  const set = (patch: Partial<IndexSpec>) => d.setIdx((xs) => patchRow(xs, r.key, patch));
  const status = r.dropped ? "dropped" : !r.orig ? "added" : "";
  return (
    <tr className={status}>
      <td>
        {r.orig ? (
          <span className="row">
            {r.orig.primary && <KeyRound size={12} color="var(--warn)" />}
            <span className="selectable">{r.orig.name}</span>
          </span>
        ) : (
          <input
            className="cell-input"
            placeholder="generated if empty"
            value={r.cur.name}
            onChange={(e) => set({ name: e.target.value })}
          />
        )}
      </td>
      <td>
        {r.orig ? (
          <span className="mono">{r.orig.columns.join(", ")}</span>
        ) : (
          <ColumnChips
            names={liveColumnNames(d.cols)}
            value={r.cur.columns}
            onChange={(columns) => set({ columns })}
          />
        )}
      </td>
      <td className="center">
        <input
          type="checkbox"
          checked={r.cur.unique}
          disabled={!!r.orig}
          onChange={(e) => set({ unique: e.target.checked })}
        />
      </td>
      <td className="mono faint selectable ellipsis" title={r.orig?.definition}>
        {r.orig?.definition}
      </td>
      <td className="center">
        {!r.orig?.primary && (
          <DropButton
            dropped={r.dropped}
            what="index"
            onClick={() => d.setIdx((xs) => toggleDrop(xs, r.key))}
          />
        )}
      </td>
    </tr>
  );
}

export function IndexesEditor({ d }: { d: TableDesignerState }) {
  return (
    <>
      {d.idx.length === 0 ? (
        <div className="faint pad-y">No indexes yet.</div>
      ) : (
        <table className="edit-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Columns</th>
              <th className="center">Unique</th>
              <th>Definition</th>
              <th className="col-action" />
            </tr>
          </thead>
          <tbody>
            {d.idx.map((r) => (
              <IndexRowEditor key={r.key} d={d} row={r} />
            ))}
          </tbody>
        </table>
      )}
      <div className="row designer-add">
        <button className="btn sm" onClick={() => d.setIdx((xs) => [...xs, newIndexRow()])}>
          <Plus size={13} /> Add index
        </button>
        <span className="faint small">The primary key is set on the Columns page.</span>
      </div>
    </>
  );
}
