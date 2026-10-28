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

export interface DataGridProps {
  columns: GridColumn[];
  rowCount: number;
  getCell: (row: number, col: number) => GridValue;
  rowStatus?: (row: number) => RowStatus;
  isEdited?: (row: number, col: number) => boolean;
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
  onRowContextMenu?: (e: React.MouseEvent, row: number, col: number) => void;
  onCopy?: () => void;
  onDeleteRows?: () => void;
  rowNumberOffset?: number;
  emptyText?: string;
  /** Rows are on their way: a spinner instead of the empty text. */
  loading?: boolean;
  /** Column names not shown. */
  hidden?: string[];
  onHeaderContextMenu?: (e: React.MouseEvent, col: number) => void;
  /** Space on the focused cell (Quick Look). */
  onQuickLook?: (row: number, col: number) => void;
}
