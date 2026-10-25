// One table on the diagram: header, then a row per column with PK / FK markers.

import { startDrag } from "@/lib/drag";
import { type DiagramTable, type Point } from "@/types";
import { HEADER_H, ROW_H, type Rect } from "./layout";

export interface TableNodeProps {
  table: DiagramTable;
  rect: Rect;
  scale: number;
  state: "focus" | "dim" | "normal";
  onMove: (p: Point) => void;
  onMoved: () => void;
  onSelect: () => void;
  onHover: (on: boolean) => void;
  onOpen: () => void;
}

function marker(t: DiagramTable, column: string, primaryKey: boolean): string {
  if (primaryKey) return "PK";
  return t.foreignKeys.some((f) => f.columns.includes(column)) ? "FK" : "";
}

export function TableNode(p: TableNodeProps) {
  const { table: t, rect: r } = p;
  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.stopPropagation();
    let moved = false;
    startDrag(e, {
      bodyClass: "dragging-grab",
      onMove: (dx, dy) => {
        moved = moved || Math.abs(dx) + Math.abs(dy) > 2;
        if (moved) p.onMove({ x: r.x + dx / p.scale, y: r.y + dy / p.scale });
      },
      onEnd: () => (moved ? p.onMoved() : p.onSelect()),
    });
  };
  return (
    <g
      className={`diagram-table ${p.state}`}
      transform={`translate(${r.x} ${r.y})`}
      data-table={t.name}
      onMouseDown={onMouseDown}
      onDoubleClick={p.onOpen}
      onMouseEnter={() => p.onHover(true)}
      onMouseLeave={() => p.onHover(false)}
    >
      <rect className="diagram-box" width={r.w} height={r.h} rx={8} />
      <rect className="diagram-head" width={r.w} height={HEADER_H} rx={8} />
      <rect className="diagram-head" y={HEADER_H - 8} width={r.w} height={8} />
      <text className="diagram-title" x={12} y={HEADER_H / 2 + 4.5}>
        {t.name}
      </text>
      {t.columns.map((c, i) => {
        const y = HEADER_H + i * ROW_H;
        const m = marker(t, c.name, c.primaryKey);
        return (
          <g key={c.name} transform={`translate(0 ${y})`}>
            {m && (
              <text className={`diagram-key ${m.toLowerCase()}`} x={10} y={14}>
                {m}
              </text>
            )}
            <text className={`diagram-col${c.nullable ? "" : " required"}`} x={32} y={14}>
              {c.name}
            </text>
            <text className="diagram-type" x={r.w - 10} y={14} textAnchor="end">
              {c.dataType}
            </text>
          </g>
        );
      })}
    </g>
  );
}
