// The Index view of a table (TablePlus' "Index" segment): index_name, index_algorithm,
// is_unique and column_name. Existing indexes can be dropped; new ones are filled in
// place (their columns from a menu, or typed as a list).

import { useEffect, useRef, useState } from "react";
import { type ContextMenuApi, type MenuItem } from "@/components/ContextMenu";
import { cx } from "@/lib/cx";
import { store } from "@/store";
import { type Dialect, type IndexSpec } from "@/types";
import { type IdxRow, indexStatus, liveColumnNames, patchRow, toggleDrop } from "./designerModel";
import { FitIcon, GridCell } from "./StructureCells";
import { useFillerRows } from "./useGridRows";
import { type TableDesignerState } from "./useTableDesigner";

type Field = "name" | "algorithm" | "unique" | "columns";

const ALGORITHMS: Partial<Record<Dialect, string[]>> = {
  mysql: ["BTREE", "HASH"],
  postgres: ["btree", "hash", "gist", "gin", "brin", "spgist"],
};

const EMPTY = { text: "EMPTY", dim: true };

function text(r: IdxRow, field: Field, driver: Dialect) {
  const c = r.cur;
  switch (field) {
    case "name":
      return c.name ? { text: c.name } : EMPTY;
    case "algorithm":
      return { text: c.algorithm || (ALGORITHMS[driver]?.[0] ?? "") };
    case "unique":
      return { text: c.unique ? "TRUE" : "FALSE" };
    case "columns":
      return c.columns.length ? { text: c.columns.join(", ") } : EMPTY;
  }
}

export function IndexGrid(p: {
  d: TableDesignerState;
  menu: ContextMenuApi;
  readOnly: string | null;
  focus: string | null;
}) {
  const { d } = p;
  const ref = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState<{ key: string; field: Field } | null>(null);
  const filler = useFillerRows(ref, d.idx.length);
  const fields: { key: Field; label: string; popup?: boolean; center?: boolean }[] = [
    { key: "name", label: "index_name" },
    ...(ALGORITHMS[d.driver]
      ? [{ key: "algorithm" as const, label: "index_algorithm", popup: true }]
      : []),
    { key: "unique", label: "is_unique", popup: true, center: true },
    { key: "columns", label: "column_name", popup: true },
  ];

  useEffect(() => {
    if (!p.focus) return;
    setSelected(p.focus);
    setEditing({ key: p.focus, field: "name" });
  }, [p.focus]);

  const set = (key: string, patch: Partial<IndexSpec>) =>
    d.setIdx((xs) => patchRow(xs, key, patch));

  /** Existing indexes are only dropped; changing one means a new index. */
  const editable = (r: IdxRow) => {
    if (p.readOnly) {
      const title = "Can't change the indexes";
      void store.get().dialog({ kind: "alert", tone: "warning", title, message: p.readOnly });
      return false;
    }
    return !r.orig && !r.dropped;
  };

  const popup = (r: IdxRow, field: Field, cell: HTMLElement) => {
    setSelected(r.key);
    if (!editable(r)) return;
    const c = r.cur;
    let items: MenuItem[] = [];
    if (field === "algorithm") {
      items = (ALGORITHMS[d.driver] ?? []).map((a) => ({
        label: a,
        selected: (c.algorithm || ALGORITHMS[d.driver]?.[0]) === a,
        onClick: () => set(r.key, { algorithm: a }),
      }));
    } else if (field === "unique") {
      items = [true, false].map((u) => ({
        label: u ? "TRUE" : "FALSE",
        selected: c.unique === u,
        onClick: () => set(r.key, { unique: u }),
      }));
    } else if (field === "columns") {
      items = liveColumnNames(d.cols).map((name) => ({
        label: name,
        checked: c.columns.includes(name),
        onClick: () =>
          set(r.key, {
            columns: c.columns.includes(name)
              ? c.columns.filter((x) => x !== name)
              : [...c.columns, name],
          }),
      }));
    }
    if (items.length) p.menu.openOver(cell, items);
  };

  const drop = (r: IdxRow) => {
    if (p.readOnly) return;
    if (r.orig?.primary) {
      void store.get().dialog({
        kind: "alert",
        tone: "warning",
        title: "This is the primary key",
        message: "Change the primary key with the Primary field above the columns.",
      });
      return;
    }
    d.setIdx((xs) => toggleDrop(xs, r.key));
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (editing || (e.target as HTMLElement).tagName === "INPUT") return;
    const r = d.idx.find((x) => x.key === selected);
    if ((e.key === "Delete" || e.key === "Backspace") && r) {
      e.preventDefault();
      drop(r);
    }
  };

  return (
    <div className="st-scroll" ref={ref} tabIndex={0} onKeyDown={onKey}>
      <table className="st-grid" aria-label="Indexes">
        <thead>
          <tr>
            <th className="st-ruler">
              <FitIcon />
            </th>
            <th className="st-num">#</th>
            {fields.map((f) => (
              <th key={f.key}>{f.label}</th>
            ))}
            <th className="st-fill" />
          </tr>
        </thead>
        <tbody>
          {d.idx.map((r, i) => (
            <tr
              key={r.key}
              className={cx(selected === r.key && "selected", indexStatus(r))}
              onMouseDown={() => setSelected(r.key)}
              onContextMenu={(e) =>
                p.menu.open(e, [
                  {
                    label: r.dropped ? "Restore index" : "Delete index",
                    danger: !r.dropped,
                    disabled: !!r.orig?.primary || !!p.readOnly,
                    onClick: () => drop(r),
                  },
                ])
              }
            >
              <td className="st-ruler" />
              <td className="st-num">{i + 1}</td>
              {fields.map((f) => (
                <GridCell
                  key={f.key}
                  text={text(r, f.key, d.driver)}
                  popup={f.popup && !r.orig}
                  center={f.center}
                  editing={
                    editing?.key === r.key && editing.field === f.key
                      ? {
                          initial: f.key === "columns" ? r.cur.columns.join(", ") : r.cur.name,
                          placeholder: f.key === "name" ? "generated if empty" : "a, b",
                        }
                      : null
                  }
                  onEdit={() => {
                    setSelected(r.key);
                    if (!editable(r)) return;
                    if (f.key === "name" || f.key === "columns") {
                      setEditing({ key: r.key, field: f.key });
                    }
                  }}
                  onOpen={(cell) => popup(r, f.key, cell)}
                  onCommit={(t) => {
                    setEditing(null);
                    if (f.key === "name") set(r.key, { name: t.trim() });
                    else {
                      const cols = t.split(",").map((x) => x.trim());
                      set(r.key, { columns: cols.filter(Boolean) });
                    }
                  }}
                  onCancel={() => setEditing(null)}
                />
              ))}
              <td className="st-fill" />
            </tr>
          ))}
          {Array.from({ length: filler }, (_, i) => (
            <tr key={`filler-${i}`} className="st-empty" aria-hidden>
              <td className="st-ruler" />
              <td className="st-num" />
              {fields.map((f) => (
                <td key={f.key} />
              ))}
              <td className="st-fill" />
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
