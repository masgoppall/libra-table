// Keys on the focused grid: copy, select all, delete, and moving the focus.

import { type RefObject } from "react";
import { type Virtualizer } from "@tanstack/react-virtual";
import { columnLeft } from "./columns";
import { navigate, rowRange } from "./navigation";
import { type CellPos, type DataGridProps } from "./types";
import { type CellEditing } from "./useCellEditing";
import { type GridSelection } from "./useGridSelection";

interface Options {
  p: DataGridProps;
  ordered: number[];
  width: (col: number) => number;
  scrollRef: RefObject<HTMLDivElement>;
  virt: Virtualizer<HTMLDivElement, Element>;
  edit: CellEditing;
  sel: GridSelection;
  rowHeight: number;
  /** Width of the row-number column (0 when hidden). */
  numW: number;
}

const isMod = (e: React.KeyboardEvent) => e.metaKey || e.ctrlKey;

export function useGridKeyboard(o: Options) {
  const { p, ordered, width, scrollRef, virt, edit, sel } = o;
  const { focus, setFocus, anchor, setAnchor, range } = sel;

  // Keeps the focused column visible when moving with the keyboard.
  const scrollColumnIntoView = (col: number) => {
    const el = scrollRef.current;
    if (!el) return;
    const left = columnLeft(ordered, col, width, o.numW);
    const right = left + width(col);
    if (left - o.numW < el.scrollLeft) el.scrollLeft = left - o.numW;
    else if (right > el.scrollLeft + el.clientWidth) el.scrollLeft = right - el.clientWidth;
  };

  /** ⌘C, ⌘V, ⌘A, Delete and Escape; true when handled. */
  const selectionKey = (e: React.KeyboardEvent): boolean => {
    const key = e.key.toLowerCase();
    if (isMod(e) && key === "c") p.onCopy?.(range);
    else if (isMod(e) && key === "v" && p.onPaste) p.onPaste(focus, range);
    else if (isMod(e) && key === "a") {
      const all = new Set(Array.from({ length: p.rowCount }, (_, i) => i));
      sel.clearRange();
      p.onSelect(all, focus?.row ?? null);
    } else if ((e.key === "Backspace" || e.key === "Delete") && p.selected.size && p.editable) {
      p.onDeleteRows?.();
    } else if (e.key === "Escape" && (p.selected.size || range)) {
      sel.clearRange();
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
    const vertical = e.key === "ArrowDown" || e.key === "ArrowUp";
    const sideways = e.key === "ArrowLeft" || e.key === "ArrowRight";
    // Shift+←/→ starts a block of cells; Shift+↑/↓ grows it, or else the rows.
    if (e.shiftKey && (sideways || (vertical && range))) {
      sel.extendTo(to);
      scrollColumnIntoView(to.col);
    } else if (e.shiftKey && vertical && anchor !== null) {
      setFocus({ row: to.row, col: from.col });
      p.onSelect(rowRange(anchor, to.row), to.row);
    } else {
      setFocus(to);
      if (to.row !== from.row || range) {
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
    const pageRows = Math.max(1, Math.floor(height / o.rowHeight) - 2);
    // Shift+arrows move the far corner of a block; Shift+Tab still moves back.
    const extending = e.shiftKey && e.key.startsWith("Arrow");
    const from = extending && sel.head ? sel.head : focus;
    const to = navigate(
      { key: e.key, shiftKey: e.shiftKey, mod: isMod(e) },
      { focus: from, ordered, rowCount: p.rowCount, pageRows },
    );
    if (to) moveTo(e, from, to);
    else editKey(e, focus);
  };

  return { onKeyDown, scrollColumnIntoView };
}
