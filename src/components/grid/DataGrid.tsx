// Virtualized data grid (GRD-1): rows render only when visible, columns resize and
// reorder, header click sorts (GRD-3), cells edit in place (GRD-4).

import { Rows3 } from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef } from "react";
import { defaultWidth, orderColumns, ROW_H, ROWNUM_W } from "./columns";
import { GridHeader } from "./GridHeader";
import { GridRow } from "./GridRow";
import { type DataGridProps } from "./types";
import { useCellEditing } from "./useCellEditing";
import { useGridKeyboard } from "./useGridKeyboard";
import { EmptyState, LoadingState } from "../StateView";

export type { DataGridProps, GridColumn, RowStatus } from "./types";
export { defaultWidth } from "./columns";

const OVERSCAN = 12;

export function DataGrid(p: DataGridProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const ordered = useMemo(
    () => orderColumns(p.columns, p.order, p.hidden),
    [p.columns, p.order, p.hidden],
  );
  const width = (col: number) => {
    const c = p.columns[col];
    return c ? (p.widths[c.name] ?? defaultWidth(c)) : 0;
  };
  const totalWidth = ROWNUM_W + ordered.reduce((sum, i) => sum + width(i), 0);
  const virt = useVirtualizer({
    count: p.rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_H,
    overscan: OVERSCAN,
  });
  const edit = useCellEditing(p, scrollRef);
  const keys = useGridKeyboard({ p, ordered, width, scrollRef, virt, edit });

  // New columns (another page or query): forget the focus and any edit.
  useEffect(() => {
    keys.setFocus(null);
    edit.reset();
  }, [p.columns]);

  return (
    <div className="grid" ref={scrollRef} tabIndex={0} onKeyDown={keys.onKeyDown}>
      <GridHeader p={p} ordered={ordered} width={width} totalWidth={totalWidth} />
      {p.rowCount === 0 ? (
        p.loading ? (
          <LoadingState compact />
        ) : (
          <EmptyState
            compact
            art="no-rows"
            icon={<Rows3 size={18} />}
            title={p.emptyText ?? "No rows."}
          />
        )
      ) : (
        <div className="grid-body" style={{ height: virt.getTotalSize(), width: totalWidth }}>
          {virt.getVirtualItems().map((vr) => (
            <GridRow
              key={vr.key}
              p={p}
              row={vr.index}
              top={vr.start}
              ordered={ordered}
              width={width}
              totalWidth={totalWidth}
              focus={keys.focus}
              setFocus={keys.setFocus}
              edit={edit}
              onMouseDown={(e) => {
                // Right-click on a selected row keeps the selection for its menu.
                if (e.button === 2 && p.selected.has(vr.index)) return;
                keys.selectRow(vr.index, e);
              }}
            />
          ))}
        </div>
      )}
    </div>
  );
}
