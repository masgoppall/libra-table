// Console log panel (⇧⌘C): every statement the app sent, newest at the bottom.

import { Copy, Search, Trash2, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Check } from "@/components/Check";
import { EmptyState } from "@/components/StateView";
import { withShortcut } from "@/lib/actions";
import { copyText } from "@/lib/clipboard";
import { cx } from "@/lib/cx";
import { formatCount, plural } from "@/lib/format";
import { toastSuccess } from "@/services/errors";
import { useStore } from "@/store";
import { type ConsoleEntry } from "@/types";
import { filterEntries, useConsoleLog } from "./useConsoleLog";

function time(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, { hour12: false });
}

function Entry({ e }: { e: ConsoleEntry }) {
  const rows = e.rowsReturned ? plural(e.rowsReturned, "row") : `${e.rowsAffected} affected`;
  const copy = async () => {
    await copyText(e.sql);
    toastSuccess("SQL copied.");
  };
  return (
    <div className={cx("console-entry", e.slow && "slow")}>
      <span className="console-meta mono">
        {time(e.at)} · {e.elapsedMs.toFixed(1)} ms · {rows}
      </span>
      <pre className="console-sql mono selectable">{e.sql}</pre>
      <button className="btn ghost icon xs" title="Copy SQL" onClick={() => void copy()}>
        <Copy size={12} />
      </button>
    </div>
  );
}

export function ConsolePanel() {
  const setOpen = useStore((s) => s.setConsoleOpen);
  const { entries, clear } = useConsoleLog();
  const [query, setQuery] = useState("");
  const [slowOnly, setSlowOnly] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);
  const shown = useMemo(() => filterEntries(entries, query, slowOnly), [entries, query, slowOnly]);

  // Follow new entries while the list is scrolled to the bottom.
  useEffect(() => {
    const el = listRef.current;
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight < 80) el.scrollTop = el.scrollHeight;
  }, [shown.length]);

  return (
    <section className="console-panel" aria-label="Console log">
      <div className="console-head">
        <strong>Console</strong>
        <span className="faint small">{formatCount(shown.length)} statements</span>
        <div className="search-field console-search">
          <Search size={13} className="faint" />
          <input
            className="input sm"
            placeholder="Filter SQL"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <Check checked={slowOnly} onChange={setSlowOnly}>
          Slow only
        </Check>
        <span className="spacer" />
        <button className="btn sm ghost" onClick={() => void clear()}>
          <Trash2 size={13} /> Clear
        </button>
        <button
          className="btn ghost icon sm"
          onClick={() => setOpen(false)}
          title={withShortcut("Close", "toggle-console")}
          aria-label="Close console"
        >
          <X size={14} />
        </button>
      </div>
      <div className="console-list" ref={listRef}>
        {shown.length === 0 && (
          <EmptyState
            compact
            art="console"
            title="No statements yet"
            hint="Every statement LibraTable sends to the server is logged here."
          />
        )}
        {shown.map((e, i) => (
          <Entry key={`${e.at}-${i}`} e={e} />
        ))}
      </div>
    </section>
  );
}
