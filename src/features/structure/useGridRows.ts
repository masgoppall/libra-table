// What the structure grids share: empty striped rows below the last one (as an
// NSTableView draws them), and moving rows by their handle.

import { type RefObject, useEffect, useState } from "react";

/** Row and header heights of `.st-grid` (24 pt rows, 27 pt + 1 pt header). */
const ROW = 24;
const HEAD = 28;

/** How many empty rows fill the rest of the scroll area. */
export function useFillerRows(ref: RefObject<HTMLElement>, rows: number): number {
  const [height, setHeight] = useState(0);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setHeight(el.clientHeight));
    ro.observe(el);
    setHeight(el.clientHeight);
    return () => ro.disconnect();
  }, [ref]);
  return Math.max(0, Math.ceil((height - HEAD) / ROW) - rows);
}

export interface DropTarget {
  key: string;
  after: boolean;
}

/** Drag a row by its handle and drop it before or after another. */
export function useRowDrag(onMove: (key: string, target: DropTarget) => void) {
  const [dragging, setDragging] = useState<string | null>(null);
  const [target, setTarget] = useState<DropTarget | null>(null);
  const end = () => {
    setDragging(null);
    setTarget(null);
  };
  const handle = (key: string) => ({
    draggable: true,
    onDragStart: (e: React.DragEvent) => {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", key);
      setDragging(key);
    },
    onDragEnd: end,
  });
  const row = (key: string) => ({
    onDragOver: (e: React.DragEvent) => {
      if (!dragging) return;
      e.preventDefault();
      const r = e.currentTarget.getBoundingClientRect();
      const after = e.clientY > r.top + r.height / 2;
      if (target?.key !== key || target.after !== after) setTarget({ key, after });
    },
    onDrop: (e: React.DragEvent) => {
      e.preventDefault();
      if (dragging && target && target.key !== dragging) onMove(dragging, target);
      end();
    },
  });
  return { handle, row, dragging, target };
}
