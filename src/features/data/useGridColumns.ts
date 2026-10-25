// The grid's columns: type, key markers, and the in-place editor each one gets.

import { useMemo } from "react";
import { type GridColumn } from "@/components/grid/DataGrid";
import { editorFor } from "@/domain/cellEditors";
import { type Driver, type PageResult } from "@/types";
import { type TableRelations } from "./useTableRelations";

export function useGridColumns(
  driver: Driver,
  result: PageResult | null,
  relations: TableRelations,
): GridColumn[] {
  return useMemo(() => {
    const fkColumns = new Set(relations.foreignKeys.flatMap((fk) => fk.columns));
    return (result?.columns ?? []).map((c) => ({
      name: c.name,
      typeName: c.dataType,
      isPk: c.pkPosition !== null,
      isFk: fkColumns.has(c.name),
      editor: editorFor(driver, c, relations.choices[c.name]),
    }));
  }, [driver, result, relations]);
}
