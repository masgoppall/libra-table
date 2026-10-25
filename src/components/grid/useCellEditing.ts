// Editing one cell in place (GRD-4).

import { type RefObject, useRef, useState } from "react";
import { cellText, isBinary, isEmptyCell } from "@/lib/cells";
import { type DataGridProps } from "./types";

export interface Editing {
  row: number;
  col: number;
  value: string;
}

export function useCellEditing(p: DataGridProps, scrollRef: RefObject<HTMLDivElement>) {
  const [editing, setEditingState] = useState<Editing | null>(null);
  // Enter commits and then the input's blur fires too; the ref makes that one commit.
  const ref = useRef(editing);
  const setEditing = (e: Editing | null) => {
    ref.current = e;
    setEditingState(e);
  };

  const commit = () => {
    const ed = ref.current;
    if (!ed) return;
    ref.current = null;
    const current = p.getCell(ed.row, ed.col);
    // Opening a NULL cell and leaving it empty keeps it NULL (not an empty string).
    const unchanged = isEmptyCell(current) ? ed.value === "" : cellText(current) === ed.value;
    if (!unchanged) p.onEdit?.(ed.row, ed.col, ed.value);
    setEditing(null);
    scrollRef.current?.focus();
  };

  /** Drops an edit without committing or moving the focus. */
  const reset = () => setEditing(null);

  const cancel = () => {
    setEditing(null);
    scrollRef.current?.focus();
  };

  /** Starts editing; `replaceWith` is a typed character that replaces the value. */
  const begin = (row: number, col: number, replaceWith?: string): boolean => {
    if (!p.editable || p.rowStatus?.(row) === "deleted") return false;
    const v = p.getCell(row, col);
    if (isBinary(v)) return false;
    setEditing({ row, col, value: replaceWith ?? cellText(v) });
    return true;
  };

  const change = (value: string) => ref.current && setEditing({ ...ref.current, value });

  return { editing, begin, commit, cancel, reset, change };
}

export type CellEditing = ReturnType<typeof useCellEditing>;
