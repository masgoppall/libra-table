// Keys on the focused grid: copy, select all, delete, and moving the focus.

import { type RefObject, useState } from "react";
import { type Virtualizer } from "@tanstack/react-virtual";
import { columnLeft, ROW_H, ROWNUM_W } from "./columns";
import { navigate, rowRange } from "./navigation";
import { type CellPos, type DataGridProps } from "./types";
import { type CellEditing } from "./useCellEditing";

interface Options {
  p: DataGridProps;
  ordered: number[];
  width: (col: number) => number;
  scrollRef: RefObject<HTMLDivElement>;
  virt: Virtualizer<HTMLDivElement, Element>;
  edit: CellEditing;
}

const isMod = (e: React.KeyboardEvent) => e.metaKey || e.ctrlKey;

export function useGridKeyboard({ p, ordered, width, scrollRef, virt, edit }: Options) {
  const [focus, setFocus] = useState<CellPos | null>(null);
  const [anchor, setAnchor] = useState<number | null>(null);

  // Keeps the focused column visible when moving with the keyboard.
  const scrollColumnIntoView = (col: number) => {
    const el = scrollRef.current;
    if (!el) return;
    const left = columnLeft(ordered, col, width);
    const right = left + width(col);
    if (left - ROWNUM_W < el.scrollLeft) el.scrollLeft = left - ROWNUM_W;
    else if (right > el.scrollLeft + el.clientWidth) el.scrollLeft = right - el.clientWidth;
  };

  /** ⌘C, ⌘A, Delete and Escape; true when handled. */
  const selectionKey = (e: React.KeyboardEvent): boolean => {
    const key = e.key.toLowerCase();
    if (isMod(e) && key === "c") p.onCopy?.();
    else if (isMod(e) && key === "a") {
      const all = new Set(Array.from({ length: p.rowCount }, (_, i) => i));
      p.onSelect(all, focus?.row ?? null);
    } else if ((e.key === "Backspace" || e.key === "Delete") && p.selected.size && p.editable) {
      p.onDeleteRows?.();
    } else if (e.key === "Escape" && p.selected.size) {
      p.onSelect(new Set(), null);
      return true;
    } else return false;
    e.preventDefault();
    return true;
  };

  /** Arrow keys start from the first cell. */
  const startFocus = (e: React.KeyboardEvent) => {
    const first = ordered[0];
    if (p.rowCount === 0 || first === undefined || !e.key.startsWith("Arrow")) return;
    e.preventDefault();
    setFocus({ row: 0, col: first });
    p.onSelect(new Set([0]), 0);
  };

  const editKey = (e: React.KeyboardEvent, at: CellPos): boolean => {
    if (e.key === "Enter" || e.key === "F2") {
      e.preventDefault();
      edit.begin(at.row, at.col);
      return true;
    }
    // Space previews the cell (Quick Look), as in TablePlus.
    if (e.key === " " && p.onQuickLook && !isMod(e)) {
      e.preventDefault();
      p.onQuickLook(at.row, at.col);
      return true;
    }
    // Typing over a cell starts editing it, like a spreadsheet.
    if (e.key.length === 1 && !isMod(e) && !e.altKey) {
      if (edit.begin(at.row, at.col, e.key)) e.preventDefault();
      return true;
    }
    return false;
  };

  const moveTo = (e: React.KeyboardEvent, from: CellPos, to: CellPos) => {
    e.preventDefault();
    const extending = e.shiftKey && (e.key === "ArrowDown" || e.key === "ArrowUp");
    if (extending && anchor !== null) {
      setFocus({ row: to.row, col: from.col });
      p.onSelect(rowRange(anchor, to.row), to.row);
    } else {
      setFocus(to);
      if (to.row !== from.row) {
        p.onSelect(new Set([to.row]), to.row);
        setAnchor(to.row);
      }
      scrollColumnIntoView(to.col);
    }
    virt.scrollToIndex(to.row, { align: "auto" });
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (edit.editing || selectionKey(e)) return;
    if (!focus) return startFocus(e);
    const height = scrollRef.current?.clientHeight ?? 400;
    const pageRows = Math.max(1, Math.floor(height / ROW_H) - 2);
    const to = navigate(
      { key: e.key, shiftKey: e.shiftKey, mod: isMod(e) },
      { focus, ordered, rowCount: p.rowCount, pageRows },
    );
    if (to) moveTo(e, focus, to);
    else editKey(e, focus);
  };

  /** Click selection: Shift extends from the anchor, ⌘ toggles a row. */
  const selectRow = (row: number, e: React.MouseEvent) => {
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

  return { focus, setFocus, onKeyDown, selectRow };
}
