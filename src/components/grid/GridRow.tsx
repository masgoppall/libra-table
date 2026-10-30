// One row of cells, with the in-place editor on the cell being edited.

import { cellValue, displayCell, isBinary, isDefaultMarker, isNumericType } from "@/lib/cells";
import { inRange } from "./navigation";
import { type DataGridProps } from "./types";
import { CellEditor } from "./CellEditor";
import { type CellEditing } from "./useCellEditing";
import { type GridSelection } from "./useGridSelection";

const TEXT = { kind: "text" } as const;

interface RowProps {
  p: DataGridProps;
  row: number;
  top: number;
  ordered: number[];
  width: (col: number) => number;
  totalWidth: number;
  sel: GridSelection;
  edit: CellEditing;
}

function cellClass(
  p: DataGridProps,
  r: number,
  col: number,
  focused: boolean,
  ranged: boolean,
): string {
  const raw = p.getCell(r, col);
  const v = cellValue(raw);
  const type = p.columns[col]?.typeName ?? "";
  const match = p.isMatch?.(r, col);
  return [
    "grid-cell",
    isNumericType(type) && v !== null && "num",
    v === null && "null",
    isBinary(v) && "binary",
    p.isEdited?.(r, col) && "edited",
    focused && "focus",
    ranged && "in-range",
    match && "match",
    match === "current" && "current",
  ]
    .filter(Boolean)
    .join(" ");
}

export function GridRow(props: RowProps) {
  const { p, row: r, edit, ordered, sel } = props;
  const status = p.rowStatus?.(r);
  // A block of cells shows in its cells, not across its rows.
  const rowSelected = p.selected.has(r) && !sel.range;
  const classes = ["grid-row", r % 2 === 1 && "alt", rowSelected && "selected", status];

  const tabFrom = (col: number) => (back: boolean) => {
    const next = ordered[ordered.indexOf(col) + (back ? -1 : 1)];
    if (next !== undefined) sel.setFocus({ row: r, col: next });
  };

  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      style={{ top: props.top, width: props.totalWidth }}
    >
      <div
        className="grid-cell grid-rownum"
        onMouseDown={(e) => {
          if (e.button !== 2 || !p.selected.has(r)) sel.selectRow(r, e);
        }}
      >
        {(p.rowNumberOffset ?? 0) + r + 1}
      </div>
      {ordered.map((col) => {
        const raw = p.getCell(r, col);
        const editing = edit.editing?.row === r && edit.editing.col === col;
        const focused = sel.focus?.row === r && sel.focus.col === col;
        const ranged = inRange(sel.range, r, col);
        return (
          <div
            key={p.columns[col]?.name ?? col}
            className={cellClass(p, r, col, focused, ranged)}
            style={{ width: props.width(col) }}
            onMouseDown={(e) => sel.cellDown({ row: r, col }, e)}
            onMouseEnter={() => sel.cellEnter({ row: r, col })}
            onDoubleClick={() => edit.begin(r, col)}
            onContextMenu={(e) => p.onRowContextMenu?.(e, r, col, ranged ? sel.range : null)}
          >
            {editing && edit.editing ? (
              <CellEditor
                edit={edit}
                value={edit.editing.value}
                editor={p.columns[col]?.editor ?? TEXT}
                onTab={tabFrom(col)}
              />
            ) : isDefaultMarker(raw) ? (
              "DEFAULT"
            ) : (
              displayCell(raw)
            )}
          </div>
        );
      })}
    </div>
  );
}
