// Columns page of the designer: name, type, NULL, default, PK, auto, unique, comment.

import { Plus } from "lucide-react";
import { withShortcut } from "@/lib/actions";
import { type ColumnSpec } from "@/types";
import { columnStatus, type ColRow, editColumn, toggleDrop } from "./designerModel";
import { DropButton } from "./DropButton";
import { type TableDesignerState } from "./useTableDesigner";

function Toggle(props: {
  checked: boolean;
  disabled?: boolean;
  title?: string;
  onChange: (v: boolean) => void;
}) {
  return (
    <td className="center">
      <input
        type="checkbox"
        checked={props.checked}
        disabled={props.disabled}
        title={props.title}
        onChange={(e) => props.onChange(e.target.checked)}
      />
    </td>
  );
}

function ColumnRowEditor(props: { d: TableDesignerState; row: ColRow; index: number }) {
  const { d, row: r } = props;
  const c = r.cur;
  const off = r.dropped;
  const set = (patch: Partial<ColumnSpec>) =>
    d.setCols((rows) =>
      rows.map((x) => (x.key === r.key ? { ...x, cur: editColumn(x.cur, patch) } : x)),
    );
  const invalid = (empty: boolean) => (empty && !off ? " invalid" : "");
  return (
    <tr className={columnStatus(r)} data-col={r.key}>
      <td className="faint num">{props.index + 1}</td>
      <td>
        <input
          className={`cell-input${invalid(!c.name.trim())}`}
          value={c.name}
          disabled={off}
          placeholder="column_name"
          onChange={(e) => set({ name: e.target.value })}
        />
      </td>
      <td>
        <input
          className={`cell-input mono${invalid(!c.dataType.trim())}`}
          list={`lt-types-${d.driver}`}
          value={c.dataType}
          disabled={off}
          onChange={(e) => set({ dataType: e.target.value })}
        />
      </td>
      <Toggle
        checked={c.nullable}
        disabled={off || c.primaryKey}
        onChange={(nullable) => set({ nullable })}
      />
      <td>
        <input
          className="cell-input mono"
          list={`lt-defaults-${d.driver}`}
          value={c.default ?? ""}
          placeholder={c.autoIncrement ? "auto" : "none"}
          disabled={off || (c.autoIncrement && d.driver !== "sqlite")}
          onChange={(e) => set({ default: e.target.value })}
        />
      </td>
      <Toggle
        checked={c.primaryKey}
        disabled={off}
        onChange={(primaryKey) => set({ primaryKey })}
      />
      <Toggle
        checked={c.autoIncrement}
        disabled={off}
        onChange={(autoIncrement) => set({ autoIncrement })}
      />
      <Toggle
        checked={c.unique}
        disabled={off || !!r.orig}
        title={r.orig ? "Add a unique index under Indexes" : undefined}
        onChange={(unique) => set({ unique })}
      />
      {d.driver !== "sqlite" && (
        <td>
          <input
            className="cell-input"
            value={c.comment ?? ""}
            disabled={off}
            onChange={(e) => set({ comment: e.target.value })}
          />
        </td>
      )}
      <td className="center">
        <DropButton
          dropped={r.dropped}
          what="column"
          onClick={() => d.setCols((rows) => toggleDrop(rows, r.key))}
        />
      </td>
    </tr>
  );
}

function ColumnsHead({ comments }: { comments: boolean }) {
  return (
    <thead>
      <tr>
        <th className="col-num">#</th>
        <th>Name</th>
        <th>Type</th>
        <th className="center" title="Allows NULL">
          Null
        </th>
        <th title="SQL expression, e.g. 0, 'text', now()">Default</th>
        <th className="center" title="Primary key">
          PK
        </th>
        <th className="center" title="Auto increment / identity">
          Auto
        </th>
        <th className="center" title="Unique (new columns)">
          Unique
        </th>
        {comments && <th>Comment</th>}
        <th className="col-action" />
      </tr>
    </thead>
  );
}

export function ColumnsEditor({ d }: { d: TableDesignerState }) {
  const comments = d.driver !== "sqlite";
  return (
    <>
      {d.creating && comments && (
        <div className="row designer-meta">
          <span className="muted">Comment</span>
          <input
            className="input sm grow comment-input"
            placeholder="optional description of the table"
            value={d.comment}
            onChange={(e) => d.setComment(e.target.value)}
          />
        </div>
      )}
      <table className="edit-table">
        <ColumnsHead comments={comments} />
        <tbody>
          {d.cols.map((r, i) => (
            <ColumnRowEditor key={r.key} d={d} row={r} index={i} />
          ))}
        </tbody>
      </table>
      <div className="row designer-add">
        <button
          className="btn sm"
          onClick={d.addColumn}
          title={withShortcut("Add column", "add-row")}
        >
          <Plus size={13} /> Add column
        </button>
        {d.driver === "sqlite" && !d.creating && (
          <span className="faint small">
            Changing a type, NULL or default rebuilds the table (rows, indexes and constraints are
            kept).
          </span>
        )}
      </div>
    </>
  );
}
