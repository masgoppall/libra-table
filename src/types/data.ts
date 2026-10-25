// Table data: pages, filters, sorting, cell values and edits.

import { type ColumnInfo } from "./schema";

export interface FilterRow {
  enabled: boolean;
  column: string;
  operator: string;
  value: string;
  value2: string;
}

export interface SortSpec {
  column: string;
  desc: boolean;
}

export interface PageRequest {
  schema: string;
  table: string;
  page: number;
  pageSize: number;
  sort: SortSpec[];
  filters: FilterRow[];
  /** Index of the only filter to apply; null applies every checked filter. */
  applyOnly?: number | null;
}

/** A cell as sent by Rust: null, boolean, number, string, or binary preview. */
export interface BinaryCell {
  $binary: string;
  size: number;
}
export type Cell = null | boolean | number | string | BinaryCell;

/** Marks a cell that will take the column default (new rows, "Set DEFAULT"). */
export interface DefaultMarker {
  $default: true;
}

/** What a grid shows: a value, or the column default for pending edits. */
export type GridValue = Cell | DefaultMarker;

/** Cell value sent back for edits. */
export type CellInput = null | string | number | boolean | DefaultMarker;

export interface PageResult {
  columns: ColumnInfo[];
  rows: Cell[][];
  primaryKey: string[];
  page: number;
  pageSize: number;
  hasMore: boolean;
  sql: string;
  elapsedMs: number;
}

export interface RowCount {
  total: number;
  exact: boolean;
}

export interface FilterPreview {
  row: string | null;
  all: string;
}

export type Change =
  | { kind: "update"; pk: Record<string, Cell>; values: Record<string, CellInput> }
  | { kind: "insert"; values: Record<string, CellInput> }
  | { kind: "delete"; pk: Record<string, Cell> };

export interface CommitResult {
  affected: number;
  statements: string[];
}

/** Per-table view state saved between sessions (STO-4). */
export interface TableState {
  columnOrder: string[];
  columnWidths: Record<string, number>;
  filters: FilterRow[];
  sort: SortSpec[];
  /** Columns hidden in the grid. */
  hiddenColumns?: string[];
}

/** One whole cell for Quick Look (binary values are only previewed in the grid). */
export type CellContent =
  | { kind: "null" }
  | { kind: "text"; text: string }
  | { kind: "binary"; base64: string; size: number; truncated: boolean; mime: string | null };

/** One cell of a table, found by its row's primary key. */
export interface CellTarget {
  schema: string;
  table: string;
  pk: Record<string, Cell>;
  column: string;
}

/** How a cell is edited in place in the grid. */
export type CellEditorKind =
  { kind: "text" } | { kind: "choice"; options: string[] } | { kind: "date" };
