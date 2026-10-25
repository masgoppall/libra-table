// Query history tab (QRY-6): search, reopen in an editor, run again.

import { Search, Trash2 } from "lucide-react";
import { api } from "@/api";
import { Spinner } from "@/components/Spinner";
import { LAYER, useActions } from "@/lib/actions";
import { plural } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { store } from "@/store";
import { type HistoryEntry, type Workspace } from "@/types";
import { newQueryTab } from "../workspace/tabs";
import { HistoryItem } from "./HistoryItem";
import { type HistoryScope, useHistorySearch } from "./useHistorySearch";

async function purgeOlder(): Promise<boolean> {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Delete old history",
    label: "Delete entries older than (days)",
    initial: "90",
  });
  const days = Number(r.ok ? r.value : NaN);
  if (!(days >= 1)) return false;
  try {
    const n = await api.purgeHistory(days);
    toastSuccess(`Deleted ${plural(n, "entry", "entries")} older than ${days} days.`);
    return true;
  } catch (e) {
    showError(e);
    return false;
  }
}

function HistoryToolbar({ h }: { h: ReturnType<typeof useHistorySearch> }) {
  const f = h.filters;
  return (
    <div className="toolbar">
      <div className="search-field">
        <Search size={13} className="faint" />
        <input
          className="input sm"
          placeholder="Search SQL"
          value={f.text}
          onChange={(e) => h.setFilter({ text: e.target.value })}
          autoFocus
        />
      </div>
      <select
        className="select sm"
        value={f.scope}
        onChange={(e) => h.setFilter({ scope: e.target.value as HistoryScope })}
      >
        <option value="this">This connection</option>
        <option value="all">All connections</option>
      </select>
      <span className="muted">From</span>
      <input
        className="input sm"
        type="date"
        value={f.from}
        onChange={(e) => h.setFilter({ from: e.target.value })}
      />
      <span className="muted">to</span>
      <input
        className="input sm"
        type="date"
        value={f.to}
        onChange={(e) => h.setFilter({ to: e.target.value })}
      />
      <span className="spacer" />
      <button className="btn sm" onClick={async () => (await purgeOlder()) && h.load(0)}>
        <Trash2 size={13} /> Delete older than…
      </button>
    </div>
  );
}

export function HistoryView({ ws, visible }: { ws: Workspace; visible: boolean }) {
  const id = ws.connectionId;
  const h = useHistorySearch(id);
  useActions(LAYER.tab, visible, { refresh: () => void h.load(0) });

  const open = (e: HistoryEntry, autoRun: boolean) => {
    const title = autoRun ? "Rerun" : "From history";
    store.get().addTab(id, newQueryTab(e.sql, { title, ...(autoRun ? { autoRun: true } : {}) }));
  };

  return (
    <div className="tab-body">
      <HistoryToolbar h={h} />
      <div className="history-list">
        {h.items.map((e) => (
          <HistoryItem
            key={e.id}
            entry={e}
            showConnection={h.filters.scope === "all"}
            canRun={e.connectionId === id}
            onOpen={(autoRun) => open(e, autoRun)}
            onDeleted={() => h.remove(e.id)}
          />
        ))}
        {h.loading && (
          <div className="message muted row">
            <Spinner /> Searching…
          </div>
        )}
        {!h.loading && h.items.length === 0 && (
          <div className="message faint">No history matches.</div>
        )}
        {!h.loading && h.more && (
          <div className="pad-sm">
            <button className="btn sm" onClick={h.loadMore}>
              Load more
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
