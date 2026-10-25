// Copying rows and the right-click menu of a data cell.

import { type ContextMenuApi } from "@/components/ContextMenu";
import { type GridColumn } from "@/components/grid/DataGrid";
import { cellValue } from "@/lib/cells";
import { copyText } from "@/lib/clipboard";
import { copyRows, type CopyFormat } from "@/lib/copyRows";
import { plural } from "@/lib/format";
import { toastSuccess } from "@/services/errors";
import { type Cell, type CellInput, type Driver, type GridValue } from "@/types";
import { cellMenu } from "./cellMenu";
import { referenceOf, type TableRelations } from "./useTableRelations";

export interface CellActionsContext {
  driver: Driver;
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
  quickLook: (r: number, c: number) => void;
  openReference: (ref: { table: string; column: string }, value: Cell) => void;
  menu: ContextMenuApi;
}

export function useCellActions(x: CellActionsContext) {
  const copy = async (format: CopyFormat = "tsv") => {
    if (x.selected.size === 0) return;
    const rows = [...x.selected]
      .sort((a, b) => a - b)
      .map((r) => x.columns.map((_, c) => cellValue(x.getCell(r, c))));
    const names = x.columns.map((c) => c.name);
    const opts = { driver: x.driver, table: x.qualifiedTable, header: format === "csv" };
    await copyText(copyRows(format, names, rows, opts));
    toastSuccess(`Copied ${plural(x.selected.size, "row")}.`);
  };

  const openMenu = (e: React.MouseEvent, r: number, c: number) => {
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
        editable: x.editable,
        markedDeleted: x.isDeleted(r),
        copy: (f) => void copy(f),
        setValue: (v) => x.onEdit(r, c, v),
        filterBy: (v) => x.filterBy(column, v),
        deleteRows: x.deleteRows,
        quickLook: () => x.quickLook(r, c),
        reference:
          ref && target !== null
            ? { table: ref.table, open: () => x.openReference(ref, target) }
            : undefined,
      }),
    );
  };

  return { copy, openMenu };
}
