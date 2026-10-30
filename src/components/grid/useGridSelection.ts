// What is selected in the grid: the focused cell, whole rows (click, Shift+click, ⌘+click
// on the rows) and a block of cells (drag across cells, Shift+←/→, then Shift+↑/↓), as in
// TablePlus. A block selects its rows too, so row actions keep working on it.

import { useEffect, useMemo, useRef, useState } from "react";
import { rangeRows, rowRange, spanRange } from "./navigation";
import { type CellPos, type DataGridProps } from "./types";

interface Span {
  anchor: CellPos;
  head: CellPos;
}

export function useGridSelection(p: DataGridProps, ordered: number[]) {
  const [focus, setFocusState] = useState<CellPos | null>(null);
  const [anchor, setAnchor] = useState<number | null>(null);
  const [span, setSpan] = useState<Span | null>(null);
  const dragging = useRef(false);
  const range = useMemo(() => span && spanRange(span.anchor, span.head, ordered), [span, ordered]);

  useEffect(() => {
    const up = () => (dragging.current = false);
    window.addEventListener("mouseup", up);
    return () => window.removeEventListener("mouseup", up);
  }, []);

  // Rows deselected from outside (Escape, a reload, a delete): the block goes too.
  useEffect(() => {
    if (p.selected.size === 0) setSpan(null);
  }, [p.selected]);

  const setFocus = (f: CellPos | null) => {
    setFocusState(f);
    setSpan(null);
  };

  /** Moves the far corner of the block (from the focused cell when there is none). */
  const extendTo = (head: CellPos) => {
    const from = span?.anchor ?? focus;
    if (!from) return;
    const next = { anchor: from, head };
    setSpan(next);
    const r = spanRange(next.anchor, next.head, ordered);
    p.onSelect(r ? rangeRows(r) : new Set([head.row]), from.row);
  };

  /** Click selection of rows: Shift extends from the anchor, ⌘ toggles a row. */
  const selectRow = (row: number, e: React.MouseEvent) => {
    setSpan(null);
    if (e.shiftKey && anchor !== null) {
      p.onSelect(rowRange(anchor, row), row);
      return;
    }
    let next = new Set([row]);
    if (e.metaKey || e.ctrlKey) {
      next = new Set(p.selected);
      if (next.has(row)) next.delete(row);
      else next.add(row);
    }
    setAnchor(row);
    p.onSelect(next, row);
  };

  const cellDown = (cell: CellPos, e: React.MouseEvent) => {
    if (e.button === 2) {
      // Right-click inside the selection keeps it for the menu.
      if (p.selected.has(cell.row)) return;
      setFocus(cell);
      selectRow(cell.row, e);
      return;
    }
    if (e.shiftKey && range) return extendTo(cell);
    setFocusState(cell);
    selectRow(cell.row, e);
    if (!e.shiftKey && !e.metaKey && !e.ctrlKey) {
      setSpan({ anchor: cell, head: cell });
      dragging.current = true;
    }
  };

  /** Dragging over cells grows the block. */
  const cellEnter = (cell: CellPos) => {
    if (dragging.current && span) extendTo(cell);
  };

  return {
    focus,
    setFocus,
    anchor,
    setAnchor,
    range,
    /** The far corner of the block, where Shift+arrows move from. */
    head: span?.head ?? focus,
    extendTo,
    clearRange: () => setSpan(null),
    selectRow,
    cellDown,
    cellEnter,
  };
}

export type GridSelection = ReturnType<typeof useGridSelection>;
