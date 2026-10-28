// The sidebar's "History" tab: the latest statements of this connection, filtered by the
// sidebar search. A click opens one in a new query tab; the full history is a tab.

import { useEffect } from "react";
import { Spinner } from "@/components/Spinner";
import { formatTime } from "@/lib/format";
import { store } from "@/store";
import { type HistoryEntry, type Workspace } from "@/types";
import { useHistorySearch } from "../history/useHistorySearch";
import { historyTab, newQueryTab } from "../workspace/tabs";
import { SideEmpty } from "./SideSection";

const firstLine = (sql: string) => sql.trim().split("\n")[0] ?? "";

export function SidebarHistory({ ws, query }: { ws: Workspace; query: string }) {
  const h = useHistorySearch(ws.connectionId);
  const { setFilter } = h;
  useEffect(() => setFilter({ text: query }), [query]);
  const id = ws.connectionId;
  const open = (e: HistoryEntry) =>
    store.get().addTab(id, newQueryTab(e.sql, { title: firstLine(e.sql).slice(0, 40) }));

  if (h.loading && h.items.length === 0) {
    return (
      <div className="side-loading">
        <Spinner size={16} />
      </div>
    );
  }
  if (h.items.length === 0) {
    return (
      <SideEmpty>
        {query ? `No statements match “${query}”.` : "Statements you run appear here."}
      </SideEmpty>
    );
  }
  return (
    <div className="side-flat">
      {h.items.map((e) => (
        <div
          key={e.id}
          className={`side-history ${e.status}`}
          onClick={() => open(e)}
          title={e.error ?? e.sql}
        >
          <span className="side-history-sql">{firstLine(e.sql)}</span>
          <span className="side-history-time">{formatTime(e.startedAt)}</span>
        </div>
      ))}
      <button
        className="side-link"
        onClick={() => store.get().addTab(id, historyTab(), { reuse: true })}
      >
        Show full history…
      </button>
    </div>
  );
}
