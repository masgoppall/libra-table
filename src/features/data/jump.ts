// "Jump to column or row" (TablePlus' ⌘J-style finder over a grid): a column by name, a
// row by the number shown, or a cell by its value (">" searches only the values).

import { fuzzyMatch, type Match } from "@/lib/fuzzy";

export type JumpTarget =
  | { kind: "column"; col: number; name: string; match: Match }
  | { kind: "row"; row: number; label: number }
  | { kind: "value"; row: number; col: number; column: string; text: string };

export interface JumpSource {
  columns: string[];
  /** Column indexes as displayed. */
  visible: number[];
  rowCount: number;
  /** The number shown for row 0 minus one (the page's offset). */
  rowOffset: number;
  text: (row: number, col: number) => string;
}

const VALUE_LIMIT = 50;

function valueMatches(src: JumpSource, needle: string): JumpTarget[] {
  const out: JumpTarget[] = [];
  const lower = needle.toLowerCase();
  for (let row = 0; row < src.rowCount && out.length < VALUE_LIMIT; row++) {
    for (const col of src.visible) {
      const text = src.text(row, col);
      if (!text.toLowerCase().includes(lower)) continue;
      out.push({ kind: "value", row, col, column: src.columns[col] ?? "", text });
      if (out.length >= VALUE_LIMIT) break;
    }
  }
  return out;
}

export function jumpTargets(src: JumpSource, query: string): JumpTarget[] {
  const q = query.trim();
  if (q.startsWith(">")) {
    const needle = q.slice(1).trim();
    return needle ? valueMatches(src, needle) : [];
  }
  const columns: Extract<JumpTarget, { kind: "column" }>[] = [];
  for (const col of src.visible) {
    const name = src.columns[col] ?? "";
    const match = fuzzyMatch(q, name);
    if (match) columns.push({ kind: "column", col, name, match });
  }
  if (q) columns.sort((a, b) => b.match.score - a.match.score);
  const rows: JumpTarget[] = [];
  if (/^\d+$/.test(q)) {
    const label = Number(q);
    const row = label - src.rowOffset - 1;
    if (row >= 0 && row < src.rowCount) rows.push({ kind: "row", row, label });
  }
  const values = q.length >= 2 ? valueMatches(src, q) : [];
  return [...rows, ...columns, ...values];
}
