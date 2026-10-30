// State of the "Search and Replace Within Table" bar of a data tab.

import { createElement, useMemo, useState } from "react";
import { type CellPos } from "@/components/grid/types";
import { cellText, cellValue, isBinary } from "@/lib/cells";
import { plural } from "@/lib/format";
import { toastInfo, toastSuccess } from "@/services/errors";
import { type CellInput, type GridValue } from "@/types";
import { FindBar } from "./FindBar";
import {
  cellMatches,
  findMatches,
  findPattern,
  type FindOptions,
  nextMatch,
  replaceIn,
} from "./findReplace";

export interface FindContext {
  columns: string[];
  visible: number[];
  rowCount: number;
  getCell: (r: number, c: number) => GridValue;
  isDeleted: (r: number) => boolean;
  editable: boolean;
  /** Sets cells in one undoable step. */
  setCells: (cells: { row: number; col: number; value: CellInput }[]) => void;
  go: (at: CellPos) => void;
}

const NO_OPTIONS: FindOptions = {
  query: "",
  matchCase: false,
  wholeWord: false,
  regex: false,
  column: null,
};

function searchable(v: GridValue): string | null {
  const c = cellValue(v);
  return c === null || isBinary(c) ? null : cellText(v);
}

export function useFindReplace(x: FindContext) {
  const [open, setOpen] = useState(false);
  const [signal, setSignal] = useState(0);
  const [options, setOptions] = useState(NO_OPTIONS);
  const [replacement, setReplacement] = useState("");
  const [showReplace, setShowReplace] = useState(false);
  const [current, setCurrent] = useState<CellPos | null>(null);

  const { re, error } = useMemo(() => {
    try {
      return { re: findPattern(options), error: null };
    } catch {
      return { re: null, error: "Not a valid regular expression" };
    }
  }, [options]);
  const text = (r: number, c: number) => (x.isDeleted(r) ? null : searchable(x.getCell(r, c)));
  const source = { rowCount: x.rowCount, visible: x.visible, text };
  const matches = open && re ? findMatches(source, re, options.column) : [];
  const index = current
    ? matches.findIndex((m) => m.row === current.row && m.col === current.col)
    : -1;
  const at = matches[index];

  const show = (replace: boolean) => {
    setOpen(true);
    setShowReplace((s) => s || replace);
    setSignal((n) => n + 1);
  };
  const moveTo = (m: CellPos | undefined) => {
    setCurrent(m ?? null);
    if (m) x.go(m);
  };
  const step = (back: boolean) => moveTo(matches[nextMatch(matches, current, back)]);
  const replaced = (m: CellPos) => {
    const t = text(m.row, m.col);
    return re && t !== null ? replaceIn(t, re, replacement, options.regex) : null;
  };
  const replaceOne = () => {
    if (!at || !x.editable) return;
    const value = replaced(at);
    if (value !== null) x.setCells([{ row: at.row, col: at.col, value }]);
    const rest = matches.filter((m) => m !== at);
    moveTo(rest[nextMatch(rest, at)]);
  };
  const replaceAll = () => {
    if (!x.editable) {
      return toastInfo("This table can't be edited here.");
    }
    const cells = matches.flatMap((m) => {
      const value = replaced(m);
      return value === null ? [] : [{ row: m.row, col: m.col, value }];
    });
    x.setCells(cells);
    setCurrent(null);
    toastSuccess(`Replaced ${plural(cells.length, "cell")}.`, "Commit with ⌘S to save them.");
  };

  const isMatch = (r: number, c: number): "current" | boolean => {
    if (!open || !re) return false;
    if (options.column !== null && options.column !== c) return false;
    const t = text(r, c);
    if (t === null || !cellMatches(re, t)) return false;
    return at?.row === r && at.col === c ? "current" : true;
  };

  const node =
    open &&
    createElement(FindBar, {
      options,
      onOptions: (patch) => {
        setOptions((o) => ({ ...o, ...patch }));
        setCurrent(null);
      },
      replacement,
      onReplacement: setReplacement,
      showReplace,
      onShowReplace: setShowReplace,
      columns: x.visible.map((index) => ({ index, name: x.columns[index] ?? "" })),
      count: matches.length,
      current: index,
      error,
      canReplace: x.editable,
      focusSignal: signal,
      onStep: step,
      onReplace: replaceOne,
      onReplaceAll: replaceAll,
      onClose: () => {
        setOpen(false);
        setCurrent(null);
      },
    });

  return { show, node, isMatch };
}
