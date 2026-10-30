// Search and replace within the loaded rows of a table (TablePlus' "Search and Replace
// Within Table"): plain text or a regular expression, case and whole-word options, in
// every column or one. Pure: the grid's cells come in as text.

import { type CellPos } from "@/components/grid/types";

export interface FindOptions {
  query: string;
  matchCase: boolean;
  wholeWord: boolean;
  regex: boolean;
  /** One column index, or null for every shown column. */
  column: number | null;
}

export interface FindSource {
  rowCount: number;
  /** Column indexes as displayed. */
  visible: number[];
  /** The cell as text, or null when it can't be searched (NULL, binary). */
  text: (row: number, col: number) => string | null;
}

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** The pattern, null for an empty query; throws on an invalid regular expression. */
export function findPattern(o: FindOptions): RegExp | null {
  if (!o.query) return null;
  const body = o.regex ? o.query : escape(o.query);
  const source = o.wholeWord ? `\\b(?:${body})\\b` : body;
  return new RegExp(source, o.matchCase ? "g" : "gi");
}

export function cellMatches(re: RegExp, text: string): boolean {
  re.lastIndex = 0;
  return re.test(text);
}

/** Matching cells, row by row, left to right. */
export function findMatches(src: FindSource, re: RegExp, column: number | null): CellPos[] {
  const cols = column === null ? src.visible : [column];
  const out: CellPos[] = [];
  for (let row = 0; row < src.rowCount; row++) {
    for (const col of cols) {
      const text = src.text(row, col);
      if (text !== null && cellMatches(re, text)) out.push({ row, col });
    }
  }
  return out;
}

/** Every match in `text` replaced; `$1`-style groups work with regular expressions. */
export function replaceIn(text: string, re: RegExp, replacement: string, regex: boolean): string {
  re.lastIndex = 0;
  return regex ? text.replace(re, replacement) : text.replace(re, () => replacement);
}

/** The index of the first match after `from` (wrapping around), or of the first one. */
export function nextMatch(matches: CellPos[], from: CellPos | null, back = false): number {
  if (!matches.length) return -1;
  if (!from) return back ? matches.length - 1 : 0;
  const after = (m: CellPos) => m.row > from.row || (m.row === from.row && m.col > from.col);
  const before = (m: CellPos) => m.row < from.row || (m.row === from.row && m.col < from.col);
  if (back) {
    for (let i = matches.length - 1; i >= 0; i--) if (before(matches[i] as CellPos)) return i;
    return matches.length - 1;
  }
  const i = matches.findIndex(after);
  return i < 0 ? 0 : i;
}
