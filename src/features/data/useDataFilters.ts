// Filters of a data tab (FLT-*): rows being edited ("draft") and those applied.

import { useState } from "react";
import { api } from "@/api";
import { defaultFilterColumn, newFilterRow } from "@/domain/filters";
import { cellText } from "@/lib/cells";
import { showError } from "@/services/errors";
import { useSettings } from "@/store";
import {
  type Cell,
  type Db,
  type FilterRow,
  type OperatorInfo,
  type PageRequest,
  type TableState,
} from "@/types";

export interface AppliedFilters {
  filters: FilterRow[];
  /** Only this row of `filters`; null applies every checked row. */
  only: number | null;
}

/** What "Apply" was asked for: one row, all checked rows (-1), or clear (null). */
export type ApplyTarget = number | null;

export interface FilterContext {
  connectionId: string;
  db: Db;
  operators: OperatorInfo[];
  columns: { name: string }[];
  primaryKey: string[];
  baseRequest: () => PageRequest;
  saveState: (patch: Partial<TableState>) => void;
  /** Called when the applied filters change (back to the first page). */
  onApplied: () => void;
}

export function useDataFilters(ctx: FilterContext) {
  const settings = useSettings();
  const [draft, setDraft] = useState<FilterRow[]>([]);
  const [applied, setAppliedState] = useState<AppliedFilters>({ filters: [], only: null });
  const [visible, setVisible] = useState(false);
  const [focusSignal, setFocusSignal] = useState(0);

  const activeCount = applied.only !== null ? 1 : applied.filters.filter((f) => f.enabled).length;

  const preferredOp = settings?.defaultFilterOperator ?? "eq";
  const defaultOp = ctx.operators.some((o) => o.id === preferredOp) ? preferredOp : "eq";
  const defaultColumn = defaultFilterColumn(
    settings?.defaultFilterColumn ?? "first",
    ctx.columns,
    ctx.primaryKey,
  );

  /** A row whose operator needs a value but has none filters nothing: clearing the value
   * and pressing ↵ shows every row again instead of failing. */
  const blank = (f: FilterRow) => {
    const kind = ctx.operators.find((o) => o.id === f.operator)?.valueKind ?? "single";
    if (kind === "none") return false;
    return !f.value.trim() || (kind === "range" && !f.value2.trim());
  };
  const usable = (rows: FilterRow[]) => rows.map((f) => (blank(f) ? { ...f, enabled: false } : f));

  /** From the saved table state: shown when any filter was on. */
  const restore = (s: TableState) => {
    setDraft(s.filters);
    setAppliedState({ filters: usable(s.filters), only: null });
    setVisible(s.filters.some((f) => f.enabled));
  };

  const setApplied = (next: AppliedFilters, saved: FilterRow[]) => {
    setAppliedState(next);
    ctx.saveState({ filters: saved });
    ctx.onApplied();
  };

  const applyRows = (rows: FilterRow[], only: number | null) => {
    if (only !== null && (!rows[only] || blank(rows[only]))) {
      return setApplied({ filters: [], only: null }, rows);
    }
    setApplied({ filters: usable(rows), only }, rows);
  };

  const apply = (target: ApplyTarget) => {
    if (target === null) setApplied({ filters: [], only: null }, draft);
    else applyRows(draft, target >= 0 ? target : null);
  };

  /** Removes a filter row; when filters are on, the rest apply again right away. */
  const remove = (i: number) => {
    const next = draft.filter((_, j) => j !== i);
    setDraft(next);
    if (next.length === 0) setVisible(false);
    if (activeCount > 0) applyRows(next, null);
    else ctx.saveState({ filters: next });
  };

  /** Drops every filter and shows all rows, leaving one empty row to type into. */
  const clear = () => {
    setDraft([newFilterRow(defaultColumn, defaultOp)]);
    setApplied({ filters: [], only: null }, []);
  };

  /** ⌘F shows the filters and focuses them; the button toggles them. */
  const toggle = (focus = false) => {
    if (!visible && draft.length === 0) setDraft([newFilterRow(defaultColumn, defaultOp)]);
    if (!focus) return setVisible(!visible);
    setVisible(true);
    setFocusSignal((n) => n + 1);
  };

  /** "Filter by this value" from a cell's menu. */
  const filterByValue = (column: string, value: Cell) => {
    const row: FilterRow =
      value === null
        ? newFilterRow(column, "is_null")
        : { ...newFilterRow(column, "eq"), value: cellText(value) };
    // Keep the filters that do something; drop empty rows.
    const kept = draft.filter((f) => f.value || f.operator.includes("null"));
    const next = [...kept, row];
    setDraft(next);
    setApplied({ filters: next, only: null }, next);
    setVisible(true);
  };

  /** Only rows where `column` equals `value` (following a foreign key). */
  const showOnly = (column: string, value: string) => {
    const next = [{ ...newFilterRow(column, "eq"), value }];
    setDraft(next);
    setApplied({ filters: next, only: null }, next);
    setVisible(true);
  };

  /** A new filter row on `column`, focused (from a column header). */
  const filterColumn = (column: string) => {
    setDraft([...draft.filter((f) => f.value), newFilterRow(column, defaultOp)]);
    setVisible(true);
    setFocusSignal((n) => n + 1);
  };

  const preview = async (only: number | null) => {
    try {
      const request = { ...ctx.baseRequest(), filters: draft, applyOnly: only };
      return await api.previewFilter(ctx.connectionId, ctx.db, request);
    } catch (e) {
      showError(e);
      return null;
    }
  };

  return {
    draft,
    setDraft,
    applied,
    visible,
    focusSignal,
    defaultOp,
    defaultColumn,
    activeCount,
    restore,
    apply,
    remove,
    clear,
    toggle,
    filterByValue,
    showOnly,
    filterColumn,
    preview,
  };
}

export type DataFilters = ReturnType<typeof useDataFilters>;
