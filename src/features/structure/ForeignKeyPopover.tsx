// "Create a foreign key on <column>": the referenced table and column and the ON UPDATE /
// ON DELETE rules, opened from a foreign_key cell. An existing key is replaced (dropped
// and added again under its name); one spanning several columns can only be removed.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "@/api";
import { FK_ACTIONS } from "@/domain/ddl";
import { type ForeignKeySpec } from "@/types";
import { type ColRow, columnForeignKeys, type FkRow, newForeignKeyRow } from "./designerModel";
import { type TableDesignerState } from "./useTableDesigner";

const WIDTH = 300;

function usePlacement(anchor: HTMLElement) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });
  useLayoutEffect(() => {
    const a = anchor.getBoundingClientRect();
    const h = ref.current?.offsetHeight ?? 0;
    const left = Math.max(8, Math.min(a.right - WIDTH, window.innerWidth - WIDTH - 8));
    const below = a.bottom + 4;
    const top = below + h > window.innerHeight - 8 ? Math.max(8, a.top - h - 4) : below;
    setPos({ left, top });
  }, [anchor]);
  return { ref, pos };
}

function useDismiss(ref: React.RefObject<HTMLElement>, onClose: () => void) {
  useEffect(() => {
    const down = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && onClose();
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", down);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("mousedown", down);
      window.removeEventListener("keydown", key);
    };
  }, [ref, onClose]);
}

function useColumns(d: TableDesignerState, table: string) {
  const [cols, setCols] = useState<string[]>([]);
  useEffect(() => {
    if (!table) return setCols([]);
    let live = true;
    api
      .listColumns(d.connectionId, d.db, d.schema, table)
      .then((c) => live && setCols(c.map((x) => x.name)))
      .catch(() => live && setCols([]));
    return () => {
      live = false;
    };
  }, [d.connectionId, d.db, d.schema, table]);
  return cols;
}

export function ForeignKeyPopover(p: {
  d: TableDesignerState;
  row: ColRow;
  anchor: HTMLElement;
  onClose: () => void;
}) {
  const { d } = p;
  const column = p.row.cur.name.trim();
  const fk = columnForeignKeys(d.fks, column)[0];
  const composite = !!fk && fk.cur.columns.length > 1;
  const [table, setTable] = useState(fk?.cur.refTable ?? "");
  const [refColumn, setRefColumn] = useState(fk?.cur.refColumns[0] ?? "");
  const [onUpdate, setOnUpdate] = useState(fk?.cur.onUpdate || "NO ACTION");
  const [onDelete, setOnDelete] = useState(fk?.cur.onDelete || "NO ACTION");
  const refColumns = useColumns(d, table);
  const { ref, pos } = usePlacement(p.anchor);
  useDismiss(ref, p.onClose);

  const tables = d.tables.includes(table) || !table ? d.tables : [table, ...d.tables];
  const choices = refColumns.includes(refColumn) || !refColumn ? refColumns : [refColumn];

  const remove = () => {
    if (!fk) return;
    d.setFks((xs) =>
      fk.orig
        ? xs.map((x) => (x.key === fk.key ? { ...x, dropped: true } : x))
        : xs.filter((x) => x.key !== fk.key),
    );
    p.onClose();
  };

  const save = () => {
    const spec: Partial<ForeignKeySpec> = {
      refTable: table,
      refColumns: [refColumn],
      onUpdate,
      onDelete,
    };
    d.setFks((xs) => replace(xs, fk, column, spec));
    p.onClose();
  };

  return (
    <div
      className="fkp"
      ref={ref}
      role="dialog"
      aria-label="Foreign key"
      style={{ left: pos.left, top: pos.top, width: WIDTH }}
    >
      <div className="fkp-title">
        {fk ? "Foreign key on " : "Create a foreign key on "}
        <span className="mono">{column}</span>
      </div>
      {composite ? (
        <p className="fkp-note">
          Part of <span className="mono">{fk.cur.name || "a key"}</span> (
          {fk.cur.columns.join(", ")}) referencing <span className="mono">{fk.cur.refTable}</span>.
          Remove it and create it again to change it.
        </p>
      ) : (
        <div className="fkp-grid">
          <label htmlFor="fkp-table">Table</label>
          <select
            id="fkp-table"
            value={table}
            onChange={(e) => {
              setTable(e.target.value);
              setRefColumn("");
            }}
          >
            <option value="">Choose…</option>
            {tables.map((t) => (
              <option key={t}>{t}</option>
            ))}
          </select>
          <label htmlFor="fkp-column">Column</label>
          <select
            id="fkp-column"
            value={refColumn}
            disabled={!table}
            onChange={(e) => setRefColumn(e.target.value)}
          >
            <option value="">Choose…</option>
            {choices.map((c) => (
              <option key={c}>{c}</option>
            ))}
          </select>
          <label htmlFor="fkp-update">On update</label>
          <select id="fkp-update" value={onUpdate} onChange={(e) => setOnUpdate(e.target.value)}>
            {FK_ACTIONS.map((a) => (
              <option key={a}>{a}</option>
            ))}
          </select>
          <label htmlFor="fkp-delete">On delete</label>
          <select id="fkp-delete" value={onDelete} onChange={(e) => setOnDelete(e.target.value)}>
            {FK_ACTIONS.map((a) => (
              <option key={a}>{a}</option>
            ))}
          </select>
        </div>
      )}
      <div className="fkp-foot">
        {fk && (
          <button className="sheet-btn" onClick={remove}>
            Remove
          </button>
        )}
        <span className="spacer" />
        <button className="sheet-btn" onClick={p.onClose}>
          Cancel
        </button>
        {!composite && (
          <button className="sheet-btn primary" disabled={!table || !refColumn} onClick={save}>
            Save
          </button>
        )}
      </div>
    </div>
  );
}

/** The column's key with `spec`: a new key is edited in place, an existing one is
 * dropped and added again (under its name) when something differs. */
function replace(
  xs: FkRow[],
  fk: FkRow | undefined,
  column: string,
  spec: Partial<ForeignKeySpec>,
) {
  if (!fk) {
    const row = newForeignKeyRow(column);
    return [...xs, { ...row, cur: { ...row.cur, ...spec } }];
  }
  if (!fk.orig) return xs.map((x) => (x.key === fk.key ? { ...x, cur: { ...x.cur, ...spec } } : x));
  const next = { ...fk.cur, ...spec };
  const same =
    next.refTable === fk.cur.refTable &&
    next.refColumns.join() === fk.cur.refColumns.join() &&
    next.onUpdate === fk.cur.onUpdate &&
    next.onDelete === fk.cur.onDelete;
  if (same) return xs;
  const added = newForeignKeyRow(column);
  return [
    ...xs.map((x) => (x.key === fk.key ? { ...x, dropped: true } : x)),
    { ...added, cur: { ...next, name: fk.cur.name } },
  ];
}
