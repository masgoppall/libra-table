// The keys of one Redis database: pattern and type filter, a page of keys (SCAN), and
// the value panel of the selected key.

import "./keyvalue.css";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { useState } from "react";
import { useContextMenu } from "@/components/ContextMenu";
import { PaddedError } from "@/components/ErrorCallout";
import { DataGrid, type GridColumn } from "@/components/grid/DataGrid";
import { copyText } from "@/lib/clipboard";
import { formatCount } from "@/lib/format";
import { store, tabDb, useConnection } from "@/store";
import { type Workspace, type WorkspaceTab } from "@/types";
import { PAGE_SIZES } from "../data/DataStatusBar";
import { deleteKeys, type KeyTarget, valueText } from "./edits";
import { KeysToolbar } from "./KeysToolbar";
import { NewKeyDialog } from "./NewKeyDialog";
import { type KeyRow, type RedisValue } from "./types";
import { type KeyList, useKeyList } from "./useKeyList";
import { ValuePanel } from "./ValuePanel";

const COLUMNS: GridColumn[] = [
  { name: "key", typeName: "key", isPk: true },
  { name: "type", typeName: "type" },
  { name: "ttl", typeName: "seconds" },
  { name: "size", typeName: "integer" },
  { name: "value", typeName: "preview" },
];

function cell(row: KeyRow | undefined, col: number) {
  if (!row) return null;
  return [row.key, row.type, row.ttl, row.size, row.preview][col] ?? null;
}

function StatusBar({ list }: { list: KeyList }) {
  const r = list.result;
  const n = r?.keys.length ?? 0;
  const first = list.page * list.pageSize + 1;
  return (
    <div className="statusbar">
      <span className="status-strong">
        {n > 0 ? `${formatCount(first)}–${formatCount(first + n - 1)}` : "No keys"}
        {r && <span className="muted"> · {formatCount(r.total)} keys in the database</span>}
      </span>
      <span className="faint mono ellipsis kv-scan" title={r?.query}>
        {r?.query}
      </span>
      <span className="spacer" />
      <select
        className="select xs"
        aria-label="Page size"
        value={list.pageSize}
        onChange={(e) => list.setPageSize(Number(e.target.value))}
      >
        {PAGE_SIZES.map((s) => (
          <option key={s} value={s}>
            {s} keys
          </option>
        ))}
      </select>
      <div className="pager">
        <button
          className="btn xs icon"
          aria-label="Previous page"
          disabled={list.page === 0 || list.loading}
          onClick={() => list.setPage(list.page - 1)}
        >
          <ChevronLeft size={13} />
        </button>
        <span className="pager-text">Page {list.page + 1}</span>
        <button
          className="btn xs icon"
          aria-label="Next page"
          disabled={!r?.hasMore || list.loading}
          onClick={() => list.setPage(list.page + 1)}
        >
          <ChevronRight size={13} />
        </button>
      </div>
    </div>
  );
}

export function KeysView({ ws, tab }: { ws: Workspace; tab: WorkspaceTab }) {
  const id = ws.connectionId;
  const db = tabDb(ws, tab);
  const conn = useConnection(id);
  const target: KeyTarget = { connectionId: id, db };
  const list = useKeyList(id, db, ws.refreshKey, conn?.options.pageSize ?? 300);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [current, setCurrent] = useState<RedisValue | undefined>(undefined);
  const [widths, setWidths] = useState<Record<string, number>>({});
  const [creating, setCreating] = useState(false);
  const menu = useContextMenu();
  const rows = list.result?.keys ?? [];
  const canEdit = !conn?.options.readOnly;

  const refresh = () => store.get().refresh(id);
  const selectedKeys = () => [...selected].map((i) => rows[i]?.key).filter((k) => k !== undefined);
  const remove = async (keys: RedisValue[]) => {
    if (!(await deleteKeys(target, keys))) return;
    setSelected(new Set());
    setCurrent(undefined);
    refresh();
  };

  const rowMenu = (e: React.MouseEvent, r: number) => {
    const row = rows[r];
    if (!row) return;
    const keys = selected.has(r) ? selectedKeys() : [row.key];
    menu.open(e, [
      { label: "Copy key", onClick: () => void copyText(valueText(row.key)) },
      { label: "Open in value panel", onClick: () => setCurrent(row.key) },
      { label: "", separator: true },
      {
        label: keys.length > 1 ? `Delete ${keys.length} keys` : "Delete key",
        danger: true,
        disabled: !canEdit,
        onClick: () => void remove(keys),
      },
    ]);
  };

  return (
    <div className="tab-body">
      <KeysToolbar
        filter={list.filter}
        onFilter={list.setFilter}
        loading={list.loading}
        canEdit={canEdit}
        selectedCount={selected.size}
        onReload={refresh}
        onNewKey={() => setCreating(true)}
        onDelete={() => void remove(selectedKeys())}
      />
      <PaddedError error={list.error} />
      <div className="kv-main">
        <div className="grid-wrap kv-keys">
          <DataGrid
            columns={COLUMNS}
            rowCount={rows.length}
            getCell={(r, c) => cell(rows[r], c)}
            widths={widths}
            onWidths={setWidths}
            selected={selected}
            onSelect={(sel, focus) => {
              setSelected(sel);
              const row = focus === null ? undefined : rows[focus];
              if (row) setCurrent(row.key);
            }}
            onRowContextMenu={(e, r) => rowMenu(e, r)}
            onDeleteRows={() => canEdit && void remove(selectedKeys())}
            rowNumberOffset={list.page * list.pageSize}
            emptyText="No keys match."
            loading={list.loading}
          />
        </div>
        {current !== undefined && (
          <ValuePanel
            key={valueText(current)}
            target={target}
            keyName={current}
            canEdit={canEdit}
            onChanged={refresh}
            onGone={(renamedTo) => {
              setCurrent(renamedTo);
              refresh();
            }}
            onClose={() => setCurrent(undefined)}
          />
        )}
      </div>
      <StatusBar list={list} />
      {creating && (
        <NewKeyDialog
          target={target}
          onClose={() => setCreating(false)}
          onCreated={(key) => {
            setCreating(false);
            setCurrent(key);
            refresh();
          }}
        />
      )}
      {menu.node}
    </div>
  );
}
