// The diagram itself: pure SVG, so it can be exported as drawn. Hovering or selecting a
// table highlights it, its relations and their connectors.

import { useState } from "react";
import { type DiagramTable, type DiagramLayout, type Point } from "@/types";
import { edgePath, type Rect, rectOf, related } from "./layout";
import { TableNode } from "./TableNode";
import { type Viewport } from "./useViewport";

export interface DiagramCanvasProps {
  tables: DiagramTable[];
  layout: DiagramLayout;
  viewport: Viewport;
  svgRef: React.RefObject<SVGSVGElement>;
  onMove: (name: string, p: Point) => void;
  onMoved: () => void;
  onOpen: (name: string) => void;
}

interface Edge {
  key: string;
  from: string;
  to: string;
  d: string;
}

function edges(tables: DiagramTable[], rects: Map<string, Rect>): Edge[] {
  const byName = new Map(tables.map((t) => [t.name, t]));
  return tables.flatMap((t) =>
    t.foreignKeys.flatMap((f) => {
      const from = rects.get(t.name);
      const to = rects.get(f.refTable);
      const target = byName.get(f.refTable);
      if (!from || !to || !target) return [];
      const fromRow = t.columns.findIndex((c) => c.name === f.columns[0]);
      const toRow = target.columns.findIndex((c) => c.name === f.refColumns[0]);
      const d = edgePath(from, fromRow, to, toRow);
      return [{ key: `${t.name}.${f.name}`, from: t.name, to: f.refTable, d }];
    }),
  );
}

export function DiagramCanvas(p: DiagramCanvasProps) {
  const [hover, setHover] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const focus = hover ?? selected;
  const near = focus ? related(p.tables, focus) : null;
  const rects = new Map(p.tables.map((t) => [t.name, rectOf(t, p.layout)]));
  const v = p.viewport.view;
  const state = (name: string) => (!near ? "normal" : near.has(name) ? "focus" : "dim");
  const edgeClass = (e: Edge) =>
    !focus ? "" : e.from === focus || e.to === focus ? " focus" : " dim";

  return (
    <div className="diagram-canvas" ref={p.viewport.ref}>
      <svg
        ref={p.svgRef}
        className="diagram-svg"
        width="100%"
        height="100%"
        onMouseDown={(e) => {
          setSelected(null);
          p.viewport.startPan(e);
        }}
      >
        <defs>
          <marker
            id="diagram-arrow"
            viewBox="0 0 10 10"
            refX="9"
            refY="5"
            markerWidth="7"
            markerHeight="7"
            orient="auto-start-reverse"
          >
            <path className="diagram-arrow" d="M 0 0 L 10 5 L 0 10 z" />
          </marker>
        </defs>
        <g data-diagram-content transform={`translate(${v.x} ${v.y}) scale(${v.scale})`}>
          {edges(p.tables, rects).map((e) => (
            <path
              key={e.key}
              className={`diagram-edge${edgeClass(e)}`}
              d={e.d}
              markerEnd="url(#diagram-arrow)"
            />
          ))}
          {p.tables.map((t) => (
            <TableNode
              key={t.name}
              table={t}
              rect={rects.get(t.name) ?? rectOf(t, p.layout)}
              scale={v.scale}
              state={state(t.name)}
              onMove={(pt) => p.onMove(t.name, pt)}
              onMoved={p.onMoved}
              onSelect={() => setSelected(t.name)}
              onHover={(on) => setHover(on ? t.name : null)}
              onOpen={() => p.onOpen(t.name)}
            />
          ))}
        </g>
      </svg>
    </div>
  );
}
