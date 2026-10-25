// Hidden columns, the header menu and the toolbar's Columns menu of a data tab.

import { type ContextMenuApi } from "@/components/ContextMenu";
import { type GridColumn } from "@/components/grid/DataGrid";
import { type SortSpec, type TableState } from "@/types";
import { headerMenu, visibilityMenu } from "./columnMenus";

export function useColumnControls(x: {
  columns: GridColumn[];
  state: TableState | null;
  save: (patch: Partial<TableState>) => void;
  filterColumn: (column: string) => void;
  menu: ContextMenuApi;
}) {
  const hidden = x.state?.hiddenColumns ?? [];
  const setHidden = (hiddenColumns: string[]) => x.save({ hiddenColumns });

  const openHeaderMenu = (e: React.MouseEvent, col: number) =>
    x.menu.open(
      e,
      headerMenu({
        column: x.columns[col]?.name ?? "",
        hidden,
        setHidden,
        setSort: (sort: SortSpec[]) => x.save({ sort }),
        filterColumn: x.filterColumn,
      }),
    );

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
