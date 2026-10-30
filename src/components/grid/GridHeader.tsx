// Column headers: click sorts (GRD-3), drag reorders, the edge resizes.

import { ArrowDown, ArrowUp, Key, Link2 } from "lucide-react";
import { useState } from "react";
import { startDrag } from "@/lib/drag";
import { fitWidth, MIN_W, reorder } from "./columns";
import { type DataGridProps, type GridColumn } from "./types";

const DRAG_TYPE = "text/x-col";

interface HeaderProps {
  p: DataGridProps;
  ordered: number[];
  width: (col: number) => number;
  totalWidth: number;
}

function SortMark({ p, name }: { p: DataGridProps; name: string }) {
  const at = p.sort?.findIndex((s) => s.column === name) ?? -1;
  const s = at >= 0 ? p.sort?.[at] : undefined;
  if (!s) return null;
  return (
    <span className="row sort-mark">
      {s.desc ? <ArrowDown size={12} /> : <ArrowUp size={12} />}
      {(p.sort?.length ?? 0) > 1 && <span className="faint">{at + 1}</span>}
    </span>
  );
}

function HeaderCell(
  props: HeaderProps & {
    col: number;
    dragOver: string | null;
    setDragOver: (n: string | null) => void;
  },
) {
  const { p, col } = props;
  const c: GridColumn | undefined = p.columns[col];
  if (!c) return null;
  const names = () => props.ordered.flatMap((i) => p.columns[i]?.name ?? []);
  const resize = (e: React.MouseEvent) => {
    e.stopPropagation();
    const start = props.width(col);
    startDrag(e, {
      onMove: (dx) =>
        p.onWidths?.({ ...p.widths, [c.name]: Math.max(MIN_W, Math.round(start + dx)) }),
    });
  };
  const sortHint = p.onSort ? "\nClick to sort, Shift+click to add" : "";
  const fit = (e: React.MouseEvent) => {
    e.stopPropagation();
    const grid = (e.target as HTMLElement).closest(".grid");
    const font = grid ? getComputedStyle(grid).font : "13px sans-serif";
    const w = fitWidth(c, (r) => p.getCell(r, col), p.rowCount, font);
    p.onWidths?.({ ...p.widths, [c.name]: w });
  };
  return (
    <div
      className={`grid-th${props.dragOver === c.name ? " dragover" : ""}`}
      style={{ width: props.width(col) }}
      title={`${c.name} · ${c.typeName}${sortHint}`}
      draggable={!!p.onReorder}
      onDragStart={(e) => e.dataTransfer.setData(DRAG_TYPE, c.name)}
      onDragOver={(e) => {
        if (!p.onReorder) return;
        e.preventDefault();
        props.setDragOver(c.name);
      }}
      onDragLeave={() => props.setDragOver(null)}
      onDrop={(e) => {
        props.setDragOver(null);
        const from = e.dataTransfer.getData(DRAG_TYPE);
        if (from && from !== c.name) p.onReorder?.(reorder(names(), from, c.name));
      }}
      onClick={(e) => p.onSort?.(c.name, e.shiftKey)}
      onContextMenu={(e) => p.onHeaderContextMenu?.(e, col)}
    >
      {c.isPk && <Key size={11} color="var(--warn)" />}
      <span className="th-name">{c.name}</span>
      {c.isFk && <Link2 size={11} className="th-fk" aria-label="foreign key" />}
      <span className="th-type">{c.typeName.toLowerCase()}</span>
      <SortMark p={p} name={c.name} />
      {p.onWidths && (
        <div
          className="resize"
          title="Drag to resize · double-click to fit the content"
          onMouseDown={resize}
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={fit}
        />
      )}
    </div>
  );
}

export function GridHeader(props: HeaderProps) {
  const [dragOver, setDragOver] = useState<string | null>(null);
  return (
    <div className="grid-head" style={{ width: props.totalWidth }}>
      <div className="grid-th rownum">#</div>
      {props.ordered.map((col) => (
        <HeaderCell
          key={props.p.columns[col]?.name ?? col}
          {...props}
          col={col}
          dragOver={dragOver}
          setDragOver={setDragOver}
        />
      ))}
    </div>
  );
}
