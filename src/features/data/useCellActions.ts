// Copying rows and the right-click menu of a data cell.

import { type ContextMenuApi } from "@/components/ContextMenu";
import { type GridColumn } from "@/components/grid/DataGrid";
import { type CellPos, type CellRange } from "@/components/grid/types";
import { cellValue } from "@/lib/cells";
import { copyText } from "@/lib/clipboard";
import { copyRows, type CopyFormat } from "@/lib/copyRows";
import { plural } from "@/lib/format";
import { toastSuccess } from "@/services/errors";
import { type Cell, type CellInput, type Dialect, type GridValue } from "@/types";
import { cellMenu } from "./cellMenu";
import { rememberCopy } from "./rowCopies";
import { referenceOf, type TableRelations } from "./useTableRelations";

export interface CellActionsContext {
  driver: Dialect;
  qualifiedTable: string;
  columns: GridColumn[];
  relations: TableRelations;
  selected: Set<number>;
  setSelected: (s: Set<number>) => void;
  editable: boolean;
  isDeleted: (row: number) => boolean;
  getCell: (r: number, c: number) => GridValue;
  onEdit: (r: number, c: number, v: CellInput) => void;
  filterBy: (column: string, v: Cell) => void;
  deleteRows: () => void;
  duplicate: () => void;
  paste: (at: CellPos, range: CellRange | null) => void;
  quickLook?: (r: number, c: number) => void;
  openReference: (ref: { table: string; column: string }, value: Cell) => void;
  menu: ContextMenuApi;
}

/** The values of a block of cells, row by row. */
export function rangeValues(range: CellRange, getCell: (r: number, c: number) => GridValue) {
  const rows: number[] = [];
  for (let r = range.top; r <= range.bottom; r++) rows.push(r);
  return rows.map((r) => range.cols.map((c) => cellValue(getCell(r, c))));
}

export function useCellActions(x: CellActionsContext) {
  /** The selected rows, or only the cells of a selected block. */
  const copy = async (format: CopyFormat = "tsv", range: CellRange | null = null) => {
    const opts = { driver: x.driver, table: x.qualifiedTable, header: format === "csv" };
    if (range) {
      const names = range.cols.map((c) => x.columns[c]?.name ?? "");
      await copyText(copyRows(format, names, rangeValues(range, x.getCell), opts));
      const count = (range.bottom - range.top + 1) * range.cols.length;
      return toastSuccess(`Copied ${plural(count, "cell")}.`);
    }
    if (x.selected.size === 0) return;
    const rows = [...x.selected]
      .sort((a, b) => a - b)
      .map((r) => x.columns.map((_, c) => cellValue(x.getCell(r, c))));
    const names = x.columns.map((c) => c.name);
    const text = copyRows(format, names, rows, opts);
    if (format === "tsv") rememberCopy({ text, columns: names, rows });
    await copyText(text);
    toastSuccess(`Copied ${plural(x.selected.size, "row")}.`);
  };

  const openMenu = (e: React.MouseEvent, r: number, c: number, range: CellRange | null) => {
    if (!x.selected.has(r)) x.setSelected(new Set([r]));
    const column = x.columns[c]?.name ?? "";
    const value = x.getCell(r, c);
    const ref = referenceOf(x.relations.foreignKeys, column);
    const target = cellValue(value);
    x.menu.open(
      e,
      cellMenu({
        value,
        rowCount: x.selected.has(r) ? x.selected.size : 1,
        cellCount: range ? (range.bottom - range.top + 1) * range.cols.length : 0,
        editable: x.editable,
        markedDeleted: x.isDeleted(r),
        copy: (f) => void copy(f),
        copyCells: (f) => range && void copy(f, range),
        setValue: (v) => x.onEdit(r, c, v),
        filterBy: (v) => x.filterBy(column, v),
        deleteRows: x.deleteRows,
        duplicate: x.duplicate,
        paste: () => x.paste({ row: r, col: c }, range),
        quickLook: x.quickLook && (() => x.quickLook?.(r, c)),
        reference:
          ref && target !== null
            ? { table: ref.table, open: () => x.openReference(ref, target) }
            : undefined,
      }),
    );
  };

  return { copy, openMenu };
}
