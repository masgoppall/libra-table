// Virtualized data grid (GRD-1): rows render only when visible, columns resize and
// reorder, header click sorts (GRD-3), cells edit in place (GRD-4).

import { Rows3 } from "lucide-react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef } from "react";
import { defaultWidth, orderColumns, ROWNUM_W } from "./columns";
import { useGridPrefs } from "./prefs";
import { GridHeader } from "./GridHeader";
import { GridRow } from "./GridRow";
import { type DataGridProps } from "./types";
import { useCellEditing } from "./useCellEditing";
import { useGridKeyboard } from "./useGridKeyboard";
import { useGridSelection } from "./useGridSelection";
import { EmptyState, LoadingState } from "../StateView";

export type { CellRange, DataGridProps, GridColumn, RowStatus } from "./types";
export { defaultWidth } from "./columns";

const OVERSCAN = 12;

export function DataGrid(p: DataGridProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const prefs = useGridPrefs();
  const numW = prefs.rowNumbers ? ROWNUM_W : 0;
  const ordered = useMemo(
    () => orderColumns(p.columns, p.order, p.hidden),
    [p.columns, p.order, p.hidden],
  );
  const width = (col: number) => {
    const c = p.columns[col];
    return c ? (p.widths[c.name] ?? defaultWidth(c)) : 0;
  };
  const totalWidth = numW + ordered.reduce((sum, i) => sum + width(i), 0);
  const virt = useVirtualizer({
    count: p.rowCount,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => prefs.rowHeight,
    overscan: OVERSCAN,
  });
  const edit = useCellEditing(p, scrollRef);
  const sel = useGridSelection(p, ordered);
  const keys = useGridKeyboard({ p, ordered, width, scrollRef, virt, edit, sel, ...prefs, numW });
  // A new row height re-measures the rows.
  useEffect(() => virt.measure(), [prefs.rowHeight]);

  // New columns (another page or query): forget the focus and any edit.
  useEffect(() => {
    sel.setFocus(null);
    edit.reset();
  }, [p.columns]);

  useEffect(() => {
    const at = p.reveal?.at;
    if (!at || at.row >= p.rowCount) return;
    sel.setFocus(at);
    p.onSelect(new Set([at.row]), at.row);
    virt.scrollToIndex(at.row, { align: "center" });
    keys.scrollColumnIntoView(at.col);
    scrollRef.current?.focus();
  }, [p.reveal?.seq]);

  return (
    <div
      className={["grid", !prefs.stripes && "no-stripes", !numW && "no-rownums"]
        .filter(Boolean)
        .join(" ")}
      style={{ "--row-h": `${prefs.rowHeight}px` } as React.CSSProperties}
      ref={scrollRef}
      tabIndex={0}
      onKeyDown={keys.onKeyDown}
      onDoubleClick={(e) => {
        const on = (e.target as HTMLElement).closest(".grid-row, .grid-head");
        if (!on) p.onBlankDoubleClick?.();
      }}
    >
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
              sel={sel}
              edit={edit}
            />
          ))}
        </div>
      )}
    </div>
  );
}
