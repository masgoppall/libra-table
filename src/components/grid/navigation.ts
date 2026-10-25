// Where a key moves the focused cell, spreadsheet style. Pure: no DOM, no state.

import { type CellPos } from "./types";

export interface NavContext {
  focus: CellPos;
  /** Column indexes in display order. */
  ordered: number[];
  rowCount: number;
  /** Rows per PageUp/PageDown. */
  pageRows: number;
}

export interface NavKey {
  key: string;
  shiftKey: boolean;
  mod: boolean;
}

/** The next focused cell, or null when the key doesn't move the focus. */
export function navigate(k: NavKey, c: NavContext): CellPos | null {
  const lastRow = c.rowCount - 1;
  const lastCol = c.ordered.length - 1;
  const pos = c.ordered.indexOf(c.focus.col);
  let row = c.focus.row;
  let colPos = pos;
  const right = k.key === "ArrowRight" || (k.key === "Tab" && !k.shiftKey);
  const left = k.key === "ArrowLeft" || (k.key === "Tab" && k.shiftKey);
  if (k.key === "ArrowDown") row = k.mod ? lastRow : Math.min(lastRow, row + 1);
  else if (k.key === "ArrowUp") row = k.mod ? 0 : Math.max(0, row - 1);
  else if (k.key === "PageDown") row = Math.min(lastRow, row + c.pageRows);
  else if (k.key === "PageUp") row = Math.max(0, row - c.pageRows);
  else if (k.key === "Home" && k.mod) row = 0;
  else if (k.key === "Home") colPos = 0;
  else if (k.key === "End" && k.mod) row = lastRow;
  else if (k.key === "End") colPos = lastCol;
  else if (right) colPos = k.mod ? lastCol : Math.min(lastCol, pos + 1);
  else if (left) colPos = k.mod ? 0 : Math.max(0, pos - 1);
  else return null;
  return { row, col: c.ordered[colPos] ?? c.focus.col };
}

/** Rows from `a` to `b`, inclusive, in either direction. */
export function rowRange(a: number, b: number): Set<number> {
  const out = new Set<number>();
  for (let r = Math.min(a, b); r <= Math.max(a, b); r++) out.add(r);
  return out;
}
