// One row of cells, with the in-place editor on the cell being edited.

import { cellValue, displayCell, isBinary, isDefaultMarker, isNumericType } from "@/lib/cells";
import { type CellPos, type DataGridProps } from "./types";
import { CellEditor } from "./CellEditor";
import { type CellEditing } from "./useCellEditing";

const TEXT = { kind: "text" } as const;

interface RowProps {
  p: DataGridProps;
  row: number;
  top: number;
  ordered: number[];
  width: (col: number) => number;
  totalWidth: number;
  focus: CellPos | null;
  setFocus: (f: CellPos) => void;
  edit: CellEditing;
  onMouseDown: (e: React.MouseEvent) => void;
}

function cellClass(p: DataGridProps, r: number, col: number, focused: boolean): string {
  const raw = p.getCell(r, col);
  const v = cellValue(raw);
  const type = p.columns[col]?.typeName ?? "";
  return [
    "grid-cell",
    isNumericType(type) && v !== null && "num",
    v === null && "null",
    isBinary(v) && "binary",
    p.isEdited?.(r, col) && "edited",
    focused && "focus",
  ]
    .filter(Boolean)
    .join(" ");
}

export function GridRow(props: RowProps) {
  const { p, row: r, edit, ordered } = props;
  const status = p.rowStatus?.(r);
  const classes = ["grid-row", r % 2 === 1 && "alt", p.selected.has(r) && "selected", status];

  const tabFrom = (col: number) => (back: boolean) => {
    const next = ordered[ordered.indexOf(col) + (back ? -1 : 1)];
    if (next !== undefined) props.setFocus({ row: r, col: next });
  };

  return (
    <div
      className={classes.filter(Boolean).join(" ")}
      style={{ top: props.top, width: props.totalWidth }}
      onMouseDown={props.onMouseDown}
    >
      <div className="grid-cell grid-rownum">{(p.rowNumberOffset ?? 0) + r + 1}</div>
      {ordered.map((col) => {
        const raw = p.getCell(r, col);
        const editing = edit.editing?.row === r && edit.editing.col === col;
        const focused = props.focus?.row === r && props.focus.col === col;
        return (
          <div
            key={p.columns[col]?.name ?? col}
            className={cellClass(p, r, col, focused)}
            style={{ width: props.width(col) }}
            onMouseDown={() => props.setFocus({ row: r, col })}
            onDoubleClick={() => edit.begin(r, col)}
            onContextMenu={(e) => p.onRowContextMenu?.(e, r, col)}
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
