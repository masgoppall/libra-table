// Hidden columns, the header menu and the toolbar's Columns menu of a data tab.

import { type ContextMenuApi } from "@/components/ContextMenu";
import { fitWidth } from "@/components/grid/columns";
import { type GridColumn } from "@/components/grid/DataGrid";
import { cellText, cellValue } from "@/lib/cells";
import { copyText } from "@/lib/clipboard";
import { plural } from "@/lib/format";
import { toastSuccess } from "@/services/errors";
import { type GridValue, type SortSpec, type TableState } from "@/types";
import { headerMenu, visibilityMenu } from "./columnMenus";

export function useColumnControls(x: {
  columns: GridColumn[];
  state: TableState | null;
  save: (patch: Partial<TableState>) => void;
  filterColumn: (column: string) => void;
  menu: ContextMenuApi;
  rowCount: number;
  getCell: (r: number, c: number) => GridValue;
}) {
  const hidden = x.state?.hiddenColumns ?? [];
  const setHidden = (hiddenColumns: string[]) => x.save({ hiddenColumns });

  const openHeaderMenu = (e: React.MouseEvent, col: number) => {
    const column = x.columns[col]?.name ?? "";
    const grid = (e.target as HTMLElement).closest(".grid");
    const font = grid ? getComputedStyle(grid).font : "13px sans-serif";
    const sort = x.state?.sort ?? [];
    x.menu.open(
      e,
      headerMenu({
        column,
        sorted: sort.some((s) => s.column === column),
        removeSort: () => x.save({ sort: sort.filter((s) => s.column !== column) }),
        resetLayout: () => x.save({ columnOrder: [], columnWidths: {} }),
        fit: () => {
          const c = x.columns[col];
          if (!c) return;
          const w = fitWidth(c, (r) => x.getCell(r, col), x.rowCount, font);
          x.save({ columnWidths: { ...(x.state?.columnWidths ?? {}), [column]: w } });
        },
        hidden,
        setHidden,
        setSort: (sort: SortSpec[]) => x.save({ sort }),
        filterColumn: x.filterColumn,
        copyValues: () => {
          const values = Array.from({ length: x.rowCount }, (_, r) => x.getCell(r, col));
          const lines = values.map((v) => (cellValue(v) === null ? "NULL" : cellText(v)));
          void copyText(lines.join("\n"));
          toastSuccess(`Copied ${plural(values.length, "value")}.`);
        },
      }),
    );
  };

  const openColumnsMenu = (anchor: HTMLElement) =>
    x.menu.openAt(
      anchor,
      visibilityMenu(
        x.columns.map((c) => c.name),
        hidden,
        setHidden,
      ),
    );

  return { hidden, openHeaderMenu, openColumnsMenu };
}
