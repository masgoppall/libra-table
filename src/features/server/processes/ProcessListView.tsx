// The "Processes" tab: the server's connections, auto-refresh, search, cancel / kill.

import { Activity, RefreshCw, Search } from "lucide-react";
import { PaddedError } from "@/components/ErrorCallout";
import { Check } from "@/components/Check";
import { Spinner } from "@/components/Spinner";
import { LAYER, useActions, withShortcut } from "@/lib/actions";
import { plural } from "@/lib/format";
import { type ProcessAction, type ProcessInfo, type Workspace } from "@/types";
import { REFRESH_OPTIONS, type RefreshInterval } from "./processModel";
import { ProcessTable } from "./ProcessTable";
import { stopProcess } from "./stopProcess";
import { type ProcessList, useProcessList } from "./useProcessList";

function ProcessToolbar({ list }: { list: ProcessList }) {
  return (
    <div className="toolbar">
      <span className="toolbar-title">
        <Activity size={15} />
        <strong>Processes</strong>
      </span>
      <div className="search-field">
        <Search size={13} className="faint" />
        <input
          className="input sm"
          placeholder="Search user, database, query…"
          value={list.text}
          onChange={(e) => list.setText(e.target.value)}
        />
      </div>
      <Check checked={list.hideIdle} onChange={list.setHideIdle}>
        Hide idle
      </Check>
      <span className="spacer" />
      <select
        className="select sm"
        aria-label="Auto-refresh"
        value={list.interval}
        onChange={(e) => list.setRefresh(Number(e.target.value) as RefreshInterval)}
      >
        {REFRESH_OPTIONS.map(([ms, label]) => (
          <option key={ms} value={ms}>
            {label}
          </option>
        ))}
      </select>
      <button
        className="btn sm"
        onClick={() => void list.load()}
        title={withShortcut("Refresh", "refresh")}
      >
        {list.loading ? <Spinner /> : <RefreshCw size={13} />} Refresh
      </button>
    </div>
  );
}

export function ProcessListView({ ws, visible }: { ws: Workspace; visible: boolean }) {
  const list = useProcessList(ws.connectionId, visible);
  useActions(LAYER.tab, visible, { refresh: () => void list.load() });
  const onStop = async (p: ProcessInfo, action: ProcessAction) => {
    if (await stopProcess(ws, p, action)) void list.load();
  };
  const idLabel = ws.info.driver === "postgres" ? "PID" : "ID";
  const loadedAt = list.loadedAt ? new Date(list.loadedAt).toLocaleTimeString() : null;
  return (
    <div className="tab-body">
      <ProcessToolbar list={list} />
      <PaddedError error={list.error} />
      <div className="server-scroll">
        <ProcessTable rows={list.rows} idLabel={idLabel} onStop={(p, a) => void onStop(p, a)} />
        {!list.loading && list.rows.length === 0 && (
          <div className="message faint">No processes match.</div>
        )}
      </div>
      <div className="statusbar">
        <span className="status-strong">{plural(list.rows.length, "process", "processes")}</span>
        {list.rows.length !== list.total && <span>of {list.total}</span>}
        <span className="spacer" />
        {loadedAt && <span className="faint">Updated {loadedAt}</span>}
      </div>
    </div>
  );
}
