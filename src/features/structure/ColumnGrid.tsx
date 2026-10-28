// The Structure grid of a table, as TablePlus shows it: one row per column with its
// type, character set, collation, NULL, default, extra, foreign key and comment.
// Double-click edits a cell, ⌃⌄ opens its menu, the handle moves the column (MySQL).

import { useEffect, useRef, useState } from "react";
import { type ContextMenuApi, type MenuItem, SEPARATOR } from "@/components/ContextMenu";
import { cx } from "@/lib/cx";
import { store } from "@/store";
import { type ColumnSpec } from "@/types";
import { columnPositions } from "./designerChange";
import {
  changedFields,
  type ColRow,
  editColumn,
  isTextType,
  moveRow,
  toggleDrop,
} from "./designerModel";
import * as sc from "./structureColumns";
import { FitIcon, GridCell, Grip } from "./StructureCells";
import { type DropTarget, useFillerRows, useRowDrag } from "./useGridRows";
import { type TableDesignerState } from "./useTableDesigner";

const NO_MOVE = "Currently, this database doesn't support changing ordinal position.";

interface Editing {
  key: string;
  field: sc.FieldKey;
}

export interface ColumnGridProps {
  d: TableDesignerState;
  search: string;
  menu: ContextMenuApi;
  /** Why the structure can't be edited (views…); null when it can. */
  readOnly: string | null;
  /** A row to select and start naming (a column just added). */
  focus: string | null;
  onForeignKey: (row: ColRow, cell: HTMLElement) => void;
}

function alert(message: string) {
  void store.get().dialog({ kind: "alert", tone: "warning", title: "Can't do that", message });
}

/** The fields whose cells are highlighted as changed. */
function changedCells(r: ColRow, d: TableDesignerState): Set<sc.FieldKey> {
  const spec = changedFields(r);
  const out = new Set<sc.FieldKey>();
  for (const k of spec) if (k !== "primaryKey" && k !== "unique") out.add(k as sc.FieldKey);
  const name = r.cur.name.trim();
  const fkMoved = d.fks.some(
    (f) => (f.dropped || !f.orig) && (f.orig?.columns ?? f.cur.columns).includes(name),
  );
  if (fkMoved) out.add("foreignKey");
  return out;
}

export function ColumnGrid(p: ColumnGridProps) {
  const { d } = p;
  const ref = useRef<HTMLDivElement>(null);
  const columns = sc.gridColumns(d.driver);
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const q = p.search.trim().toLowerCase();
  const rows = d.cols.filter((r) => !q || r.cur.name.toLowerCase().includes(q));
  const filler = useFillerRows(ref, rows.length);

  useEffect(() => {
    if (!p.focus) return;
    setSelected(p.focus);
    setEditing({ key: p.focus, field: "name" });
  }, [p.focus]);

  const blocked = () => {
    if (p.readOnly) alert(p.readOnly);
    return !!p.readOnly;
  };
  const set = (key: string, patch: Partial<ColumnSpec>) =>
    d.setCols((rs) => rs.map((x) => (x.key === key ? { ...x, cur: editColumn(x.cur, patch) } : x)));

  const move = (key: string, t: DropTarget) => {
    if (blocked()) return;
    const at = d.cols.findIndex((r) => r.key === t.key) + (t.after ? 1 : 0);
    const next = moveRow(d.cols, key, at);
    if (d.driver !== "mysql" && columnPositions(next).size) return alert(NO_MOVE);
    d.setCols(next);
  };
  const drag = useRowDrag(move);

  const drop = (key: string) => !blocked() && d.setCols((rs) => toggleDrop(rs, key));

  const startEdit = (r: ColRow, field: sc.FieldKey, cell?: HTMLElement) => {
    if (blocked() || r.dropped) return;
    setSelected(r.key);
    if (field === "nullable" && cell) return openPopup(r, field, cell);
    if (field === "foreignKey") return cell && p.onForeignKey(r, cell);
    // Only text types have a character set and collation.
    if ((field === "charset" || field === "collation") && !isTextType(r.cur.dataType)) return;
    setEditing({ key: r.key, field });
  };

  const openPopup = (r: ColRow, field: sc.FieldKey, cell: HTMLElement) => {
    if (blocked() || r.dropped) return;
    setSelected(r.key);
    const actions: sc.PopupActions = {
      set: (patch) => set(r.key, patch),
      manual: () => setEditing({ key: r.key, field }),
    };
    const items: Record<string, () => MenuItem[]> = {
      dataType: () => sc.typeMenu(d.driver, r.cur, actions),
      nullable: () => sc.nullableMenu(r.cur, actions),
      default: () => sc.defaultMenu(r.cur, actions),
      extra: () => sc.extraMenu(d.driver, r.cur, actions),
    };
    const build = items[field];
    if (build) p.menu.openOver(cell, build());
  };

  const rowMenu = (e: React.MouseEvent, r: ColRow) => {
    setSelected(r.key);
    const items: MenuItem[] = [
      { label: "Edit column name", onClick: () => startEdit(r, "name") },
      SEPARATOR,
      {
        label: r.dropped ? "Restore column" : "Delete column",
        danger: !r.dropped,
        onClick: () => drop(r.key),
      },
    ];
    p.menu.open(e, p.readOnly ? [] : items);
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (editing || (e.target as HTMLElement).tagName === "INPUT") return;
    const at = rows.findIndex((r) => r.key === selected);
    const step = { ArrowDown: 1, ArrowUp: -1 }[e.key];
    if (step) {
      e.preventDefault();
      const next = rows[Math.max(0, Math.min(rows.length - 1, at + step))];
      if (next) setSelected(next.key);
    } else if ((e.key === "Delete" || e.key === "Backspace") && selected) {
      e.preventDefault();
      drop(selected);
    } else if (e.key === "Enter" && selected) {
      const r = rows[at];
      if (r) startEdit(r, "name");
    } else if (e.key === "Escape") {
      setSelected(null);
    }
  };

  const number = (r: ColRow) => d.cols.indexOf(r) + 1;

  return (
    <div className="st-scroll" ref={ref} tabIndex={0} onKeyDown={onKey}>
      <table className="st-grid" aria-label="Columns">
        <thead>
          <tr>
            <th className="st-ruler">
              <FitIcon />
            </th>
            <th className="st-num">#</th>
            {columns.map((c) => (
              <th key={c.key}>{c.label}</th>
            ))}
            <th className="st-fill" />
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const changed = changedCells(r, d);
            const t = drag.target?.key === r.key ? drag.target : null;
            return (
              <tr
                key={r.key}
                data-col={r.key}
                className={cx(
                  selected === r.key && "selected",
                  !r.orig && "added",
                  r.dropped && "dropped",
                  t && (t.after ? "drop-after" : "drop-before"),
                  drag.dragging === r.key && "dragging",
                )}
                onMouseDown={() => setSelected(r.key)}
                onContextMenu={(e) => rowMenu(e, r)}
                {...drag.row(r.key)}
              >
                <td className="st-ruler" title="Drag to move the column" {...drag.handle(r.key)}>
                  <Grip />
                </td>
                <td className="st-num">{number(r)}</td>
                {columns.map((c) => {
                  const isEditing = editing?.key === r.key && editing.field === c.key;
                  const fk = c.key === "foreignKey";
                  return (
                    <GridCell
                      key={c.key}
                      text={sc.cellText(r, c.key, d.fks)}
                      popup={c.popup}
                      center={c.center}
                      arrow={fk}
                      openOnClick={fk}
                      changed={changed.has(c.key)}
                      editing={
                        isEditing
                          ? {
                              initial: sc.editText(r.cur, c.key),
                              placeholder: c.key === "name" ? "column_name" : undefined,
                            }
                          : null
                      }
                      onEdit={() => startEdit(r, c.key)}
                      onOpen={(cell) =>
                        fk ? startEdit(r, c.key, cell) : openPopup(r, c.key, cell)
                      }
                      onCommit={(text) => {
                        setEditing(null);
                        set(r.key, sc.editPatch(c.key, text));
                      }}
                      onCancel={() => setEditing(null)}
                    />
                  );
                })}
                <td className="st-fill" />
              </tr>
            );
          })}
          {Array.from({ length: filler }, (_, i) => (
            <tr key={`filler-${i}`} className="st-empty" aria-hidden>
              <td className="st-ruler" />
              <td className="st-num" />
              {columns.map((c) => (
                <td key={c.key} />
              ))}
              <td className="st-fill" />
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
