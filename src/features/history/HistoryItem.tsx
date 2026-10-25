import { Copy, Play, SquarePen, Trash2 } from "lucide-react";
import { api } from "@/api";
import { copyText } from "@/lib/clipboard";
import { formatCount, formatDuration, formatTime } from "@/lib/format";
import { showError, toastSuccess } from "@/services/errors";
import { type HistoryEntry, type HistoryStatus } from "@/types";

const STATUS_BADGE: Record<HistoryStatus, string> = {
  ok: "local",
  error: "production",
  cancelled: "staging",
};

export interface HistoryItemProps {
  entry: HistoryEntry;
  showConnection: boolean;
  /** "Run again" only for this connection's entries. */
  canRun: boolean;
  onOpen: (autoRun: boolean) => void;
  onDeleted: () => void;
}

function IconButton(props: { title: string; onClick: () => void; icon: React.ReactNode }) {
  return (
    <button className="btn sm icon ghost" title={props.title} onClick={props.onClick}>
      {props.icon}
    </button>
  );
}

export function HistoryItem(p: HistoryItemProps) {
  const h = p.entry;
  const copy = async () => {
    await copyText(h.sql);
    toastSuccess("SQL copied.");
  };
  const remove = async () => {
    await api.deleteHistory([h.id]).catch(showError);
    p.onDeleted();
  };
  return (
    <div className="history-item" onDoubleClick={() => p.onOpen(false)}>
      <div className="row history-meta">
        <span className={`badge ${STATUS_BADGE[h.status]}`}>{h.status}</span>
        <span className="muted">{formatTime(h.startedAt)}</span>
        <span className="faint">{formatDuration(h.durationMs)}</span>
        {h.rowCount !== null && <span className="faint">{formatCount(h.rowCount)} rows</span>}
        {p.showConnection && <span className="faint">· {h.connectionName}</span>}
        {h.database && <span className="faint">· {h.database}</span>}
        <span className="spacer" />
        <IconButton
          title="Open in editor"
          onClick={() => p.onOpen(false)}
          icon={<SquarePen size={13} />}
        />
        {p.canRun && (
          <IconButton title="Run again" onClick={() => p.onOpen(true)} icon={<Play size={13} />} />
        )}
        <IconButton title="Copy SQL" onClick={() => void copy()} icon={<Copy size={13} />} />
        <IconButton
          title="Delete entry"
          onClick={() => void remove()}
          icon={<Trash2 size={13} />}
        />
      </div>
      <pre className="mono selectable">{h.sql}</pre>
      {h.error && <div className="mono history-error">{h.error}</div>}
    </div>
  );
}
