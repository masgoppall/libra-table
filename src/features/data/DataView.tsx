// Table data tab: grid + filters + paging + edits + row detail (GRD-*, FLT-*).

import { useCallback, useEffect, useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { PaddedError } from "@/components/ErrorCallout";
import { orderColumns } from "@/components/grid/columns";
import { DataGrid } from "@/components/grid/DataGrid";
import { LoadingState } from "@/components/StateView";
import { store, tabDb, useConnection, useIsTabVisible } from "@/store";
import { type PageRequest, type Workspace, type WorkspaceTab } from "@/types";
import { ExportDialog } from "../export/ExportDialog";
import { switchTableView } from "../workspace/TableViewSwitch";
import { tableSaving } from "./commit";
import { DataStatusBar } from "./DataStatusBar";
import { DataToolbar, readOnlyNote } from "./DataToolbar";
import { FilterBar } from "./filters/FilterBar";
import { useGridFinders } from "./useGridFinders";
import * as edits from "./pending";
import { openReferencedRow } from "./foreignKeys";
import { RowDetail } from "./RowDetail";
import { nextSort } from "./sort";
import { useCellActions } from "./useCellActions";
import { useColumnControls } from "./useColumnControls";
import { useDataActions } from "./useDataActions";
import { useDataFilters } from "./useDataFilters";
import { useGridColumns } from "./useGridColumns";
import { useRowSelection, useTabPendingEdits } from "./usePendingEdits";
import { useQuickLook } from "./useQuickLook";
import { useRowEditing } from "./useRowEditing";
import { useTablePage } from "./useTablePage";
import { useTableRelations } from "./useTableRelations";
import { useTableState } from "./useTableState";

export function DataView({ ws, tab }: { ws: Workspace; tab: WorkspaceTab }) {
  const id = ws.connectionId;
  const schema = tab.schema ?? ws.schema;
  const table = tab.table ?? "";
  const db = tabDb(ws, tab);
  const conn = useConnection(id);
  const visible = useIsTabVisible(id, tab.id);
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(conn?.options.pageSize ?? 300);
  const [showDetail, setShowDetail] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [showSql, setShowSql] = useState(false);
  const menu = useContextMenu();
  const pendingEdits = useTabPendingEdits(tab.id);
  const { pending, dirty } = pendingEdits;
  const { selected, setSelected, focusRow, selectRows } = useRowSelection();
  const loaded = useTablePage(id, db, () => {
    pendingEdits.reset();
    selectRows(new Set(), null);
  });
  const { result } = loaded;

  const stateKey = `${id}/${tab.database ?? ws.database}.${schema}.${table}`;
  const tableState = useTableState(stateKey, (s) => filters.restore(s));
  const ts = tableState.state;

  const relations = useTableRelations(id, db, schema, table, ws.refreshKey);
  const columns = useGridColumns(ws.info.dialect, result, relations);
  const primaryKey = result?.primaryKey ?? [];
  const readOnlyConn = !!conn?.options.readOnly;
  const editable = primaryKey.length > 0 && !readOnlyConn && ws.caps.editRows;
  const baseRows = result?.rows.length ?? 0;

  // `request` is defined below; the filters only call it on demand (SQL preview).
  const filters = useDataFilters({
    connectionId: id,
    db,
    operators: ws.caps.operators,
    columns,
    primaryKey,
    baseRequest: () => request(0),
    saveState: tableState.save,
    onApplied: () => setPage(0),
  });

  const request = useCallback(
    (p: number): PageRequest => ({
      schema,
      table,
      page: p,
      pageSize,
      sort: ts?.sort ?? [],
      filters: filters.applied.filters,
      applyOnly: filters.applied.only,
    }),
    [schema, table, pageSize, ts?.sort, filters.applied],
  );

  const load = useCallback(() => {
    if (ts) void loaded.load(request(page));
  }, [loaded.load, request, page, ts !== null]);
  useEffect(load, [load, ws.refreshKey]);

  const getCell = (r: number, c: number) => (result ? edits.cellAt(pending, result, r, c) : null);
  const onEdit = (r: number, c: number, value: Parameters<typeof edits.editCell>[4]) =>
    result && pendingEdits.update((p) => edits.editCell(p, result, r, c, value));

  const { commit, reload } = tableSaving(
    { connectionId: id, db, schema, table },
    () => result && edits.buildChanges(pending, result),
    dirty,
    load,
  );

  const onSort = (column: string, additive: boolean) => {
    tableState.save({ sort: nextSort(ts?.sort ?? [], column, additive) });
    setPage(0);
  };

  const controls = useColumnControls({
    columns,
    state: ts,
    save: tableState.save,
    filterColumn: filters.filterColumn,
    menu,
    rowCount: baseRows + pending.inserts.length,
    getCell,
  });
  const visibleColumns = orderColumns(columns, ts?.columnOrder, controls.hidden);
  const rowCount = baseRows + pending.inserts.length;
  const rows = useRowEditing({
    result,
    pending,
    update: pendingEdits.update,
    getCell,
    visibleColumns,
    selected,
    select: selectRows,
    readOnlyConnection: readOnlyConn,
    editRows: ws.caps.editRows,
    editable,
  });
  const { addRow, deleteSelected } = rows;
  const finders = useGridFinders({
    columns: columns.map((c) => c.name),
    visible: visibleColumns,
    rowCount,
    rowOffset: page * pageSize,
    getCell,
    isDeleted: (r) => pending.deletes.has(r),
    editable,
    setCells: rows.setCells,
  });

  const quickLook = useQuickLook({ connectionId: id, db, schema, table }, result);
  const look = ws.caps.quickLook
    ? (r: number, c: number) => quickLook.show(r, c, getCell(r, c))
    : undefined;
  const cells = useCellActions({
    driver: ws.info.dialect,
    qualifiedTable: `${schema}.${table}`,
    columns,
    relations,
    selected,
    setSelected,
    editable,
    isDeleted: (r) => pending.deletes.has(r),
    getCell,
    onEdit,
    filterBy: filters.filterByValue,
    deleteRows: deleteSelected,
    duplicate: rows.duplicate,
    paste: (at, range) => void rows.paste(at, range),
    quickLook: look,
    openReference: (ref, value) => openReferencedRow(ws, tab, ref, value),
    menu,
  });

  // Opened from a foreign key: filter to the referenced row once the state is loaded.
  useEffect(() => {
    if (!ts || !tab.pendingFilter) return;
    filters.showOnly(tab.pendingFilter.column, tab.pendingFilter.value);
    store.get().updateTab(id, tab.id, { pendingFilter: undefined });
  }, [ts !== null, tab.pendingFilter]);

  // Like TablePlus, the tab itself shows the structure (Data | Structure).
  const openStructure = () => void switchTableView(ws, tab, "structure");

  useDataActions(visible, {
    dirty,
    canPrev: page > 0 && !loaded.loading,
    canNext: !!result?.hasMore && !loaded.loading,
    commit: () => void commit(),
    reload: () => void reload(),
    focusFilters: () => filters.toggle(true),
    addRow,
    duplicateRows: rows.duplicate,
    jump: () => finders.openJump(focusRow),
    findReplace: finders.openFind,
    discard: pendingEdits.discard,
    undo: pendingEdits.undo,
    prevPage: () => setPage(page - 1),
    nextPage: () => setPage(page + 1),
    exportRows: () => setExporting(true),
    openStructure,
    toggleDetail: () => setShowDetail((s) => !s),
  });

  const detailRow = focusRow ?? (selected.size === 1 ? ([...selected][0] ?? null) : null);

  return (
    <div className="tab-body">
      <DataToolbar
        filtersOpen={filters.visible}
        activeFilters={filters.activeCount}
        loading={loaded.loading}
        editable={editable}
        loaded={!!result}
        readOnlyNote={readOnlyConn ? "Read-only connection" : readOnlyNote(ws.caps.editRows)}
        hasSelection={selected.size > 0}
        dirty={dirty}
        detailOpen={showDetail}
        onToggleFilters={() => filters.toggle()}
        onReload={() => void reload()}
        onAddRow={addRow}
        onDelete={deleteSelected}
        onDiscard={pendingEdits.discard}
        onCommit={() => void commit()}
        onExport={() => setExporting(true)}
        onStructure={openStructure}
        onToggleDetail={() => setShowDetail(!showDetail)}
        hiddenCount={controls.hidden.length}
        onColumns={controls.openColumnsMenu}
      />
      {filters.visible && (
        <FilterBar
          columns={columns}
          operators={ws.caps.operators}
          rows={filters.draft}
          defaultOp={filters.defaultOp}
          defaultColumn={filters.defaultColumn}
          focusSignal={filters.focusSignal}
          onChange={filters.setDraft}
          onApply={filters.apply}
          onRemove={filters.remove}
          active={filters.activeCount > 0}
          onClear={filters.clear}
          onPreview={filters.preview}
        />
      )}
      {finders.findBar}
      <PaddedError error={loaded.error} />
      <div className="grid-wrap">
        {result && (
          <DataGrid
            columns={columns}
            rowCount={rowCount}
            reveal={finders.reveal}
            isMatch={finders.isMatch}
            getCell={getCell}
            rowStatus={(r) => edits.rowStatus(pending, baseRows, r)}
            isEdited={(r, c) => edits.isEdited(pending, result, r, c)}
            editable={editable}
            onEdit={onEdit}
            widths={ts?.columnWidths ?? {}}
            onWidths={(columnWidths) => tableState.save({ columnWidths })}
            order={ts?.columnOrder}
            onReorder={(columnOrder) => tableState.save({ columnOrder })}
            sort={ts?.sort}
            onSort={onSort}
            selected={selected}
            onSelect={selectRows}
            onRowContextMenu={cells.openMenu}
            onCopy={(range) => void cells.copy("tsv", range)}
            onPaste={(at, range) => void rows.paste(at, range)}
            onBlankDoubleClick={addRow}
            hidden={controls.hidden}
            onHeaderContextMenu={controls.openHeaderMenu}
            onQuickLook={look}
            onDeleteRows={deleteSelected}
            rowNumberOffset={page * pageSize}
            emptyText={filters.activeCount ? "No rows match the filters." : "This table is empty."}
          />
        )}
        {!result && loaded.loading && <LoadingState label="Loading rows…" />}
        {showDetail && result && (
          <RowDetail
            columns={columns}
            row={detailRow}
            getCell={getCell}
            editable={editable && detailRow !== null && !pending.deletes.has(detailRow)}
            onEdit={(c, v) => detailRow !== null && onEdit(detailRow, c, v)}
            onClose={() => setShowDetail(false)}
          />
        )}
      </div>
      {showSql && result && <pre className="sql-strip mono selectable">{result.sql}</pre>}
      <DataStatusBar
        result={result}
        count={loaded.count}
        page={page}
        pageSize={pageSize}
        loading={loaded.loading}
        selected={selected.size}
        showSql={showSql}
        onToggleSql={() => setShowSql(!showSql)}
        onPage={setPage}
        onPageSize={(n) => {
          setPageSize(n);
          setPage(0);
        }}
        onShowStructure={openStructure}
      />
      {menu.node}
      {quickLook.node}
      {finders.jumpSheet}
      {exporting && (
        <ExportDialog
          ws={ws}
          db={db}
          defaultName={table}
          tableSource={{ request: request(page), hasFilters: filters.activeCount > 0 }}
          onClose={() => setExporting(false)}
        />
      )}
    </div>
  );
}
