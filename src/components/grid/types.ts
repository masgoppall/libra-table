import { type CellEditorKind, type CellInput, type GridValue, type SortSpec } from "@/types";

export type { CellEditorKind } from "@/types";

export interface GridColumn {
  name: string;
  typeName: string;
  isPk?: boolean;
  /** Part of a foreign key (the header shows a link). */
  isFk?: boolean;
  editor?: CellEditorKind;
}

export type RowStatus = "inserted" | "deleted" | undefined;

export interface CellPos {
  row: number;
  col: number;
}

/** A rectangle of cells: rows `top`..`bottom` and the columns between, as displayed. */
export interface CellRange {
  top: number;
  bottom: number;
  /** Column indexes, left to right in display order. */
  cols: number[];
}

export interface DataGridProps {
  columns: GridColumn[];
  rowCount: number;
  getCell: (row: number, col: number) => GridValue;
  rowStatus?: (row: number) => RowStatus;
  isEdited?: (row: number, col: number) => boolean;
  /** Search hits ("current" for the one stepped to). */
  isMatch?: (row: number, col: number) => boolean | "current";
  editable?: boolean;
  onEdit?: (row: number, col: number, value: CellInput) => void;
  widths: Record<string, number>;
  onWidths?: (w: Record<string, number>) => void;
  order?: string[];
  onReorder?: (order: string[]) => void;
  sort?: SortSpec[];
  onSort?: (column: string, additive: boolean) => void;
  selected: Set<number>;
  onSelect: (rows: Set<number>, focusRow: number | null) => void;
  /** `range` is set when the click is inside a selected block of cells. */
  onRowContextMenu?: (
    e: React.MouseEvent,
    row: number,
    col: number,
    range: CellRange | null,
  ) => void;
  /** ⌘C; `range` is the selected block of cells, null when rows are selected. */
  onCopy?: (range: CellRange | null) => void;
  /** ⌘V, with the focused cell (null when no cell is focused) and the selected block. */
  onPaste?: (at: CellPos | null, range: CellRange | null) => void;
  /** A double-click below the rows (or on the empty grid): TablePlus adds a row. */
  onBlankDoubleClick?: () => void;
  onDeleteRows?: () => void;
  rowNumberOffset?: number;
  emptyText?: string;
  /** Rows are on their way: a spinner instead of the empty text. */
  loading?: boolean;
  /** Column names not shown. */
  hidden?: string[];
  onHeaderContextMenu?: (e: React.MouseEvent, col: number) => void;
  /** Moves the focus to a cell and scrolls it into view (Jump to column or row); a new
   * `seq` jumps again. */
  reveal?: { at: CellPos; seq: number };
  /** Space on the focused cell (Quick Look). */
  onQuickLook?: (row: number, col: number) => void;
}
