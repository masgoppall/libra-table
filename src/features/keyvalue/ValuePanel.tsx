// The selected key: name, type, size, TTL, rename and delete, and an editor for its
// value by type (string, hash, list, set, sorted set; streams are read-only).

import { Clock, Pencil, RefreshCw, Trash2, X } from "lucide-react";
import { ErrorCallout } from "@/components/ErrorCallout";
import { BusyIcon } from "@/components/Spinner";
import { formatCount } from "@/lib/format";
import { store } from "@/store";
import { CollectionEditor } from "./CollectionEditor";
import { deleteKeys, type KeyTarget, runEdit, ttlText, valueText } from "./edits";
import { StreamView } from "./StreamView";
import { StringEditor } from "./StringEditor";
import { type KeyDetail, type RedisValue } from "./types";
import { useKeyDetail } from "./useKeyDetail";

export interface ValuePanelProps {
  target: KeyTarget;
  keyName: RedisValue;
  canEdit: boolean;
  /** The value or TTL changed: the key list reloads. */
  onChanged: () => void;
  /** The key was renamed (to the new name) or deleted (undefined). */
  onGone: (renamedTo: RedisValue | undefined) => void;
  onClose: () => void;
}

async function askTtl(p: ValuePanelProps, detail: KeyDetail): Promise<boolean> {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Time to live",
    message: "Seconds until the key expires. Leave empty for no expiry.",
    label: "TTL (seconds)",
    initial: detail.ttl > 0 ? String(detail.ttl) : "",
    okLabel: "Set TTL",
  });
  if (!r.ok) return false;
  const text = (r.value ?? "").trim();
  const seconds = text === "" ? null : Number(text);
  if (seconds !== null && (!Number.isInteger(seconds) || seconds < 0)) {
    store.get().toast("error", "The TTL is a whole number of seconds.");
    return false;
  }
  const edit = { op: "expire" as const, key: p.keyName, seconds: seconds || null };
  return runEdit(p.target, edit, seconds ? `Expires in ${ttlText(seconds)}.` : "No expiry.");
}

async function askRename(p: ValuePanelProps) {
  const r = await store.get().dialog({
    kind: "prompt",
    title: "Rename key",
    label: "New name",
    initial: valueText(p.keyName),
    okLabel: "Rename",
  });
  const to = r.ok ? (r.value ?? "") : "";
  if (!to || to === p.keyName) return;
  if (await runEdit(p.target, { op: "rename", key: p.keyName, to }, "Renamed.")) p.onGone(to);
}

function Body(props: ValuePanelProps & { detail: KeyDetail; reload: () => void }) {
  const { detail } = props;
  const edit = { target: props.target, keyName: props.keyName, canEdit: props.canEdit };
  const changed = () => {
    props.reload();
    props.onChanged();
  };
  switch (detail.value.kind) {
    case "string":
      return <StringEditor {...edit} value={detail.value} onSaved={changed} />;
    case "hash":
    case "list":
    case "set":
    case "zset":
      return <CollectionEditor {...edit} value={detail.value} onSaved={changed} />;
    case "stream":
      return <StreamView entries={detail.value.entries} />;
    case "missing":
      return <div className="message faint">This key no longer exists.</div>;
    case "other":
      return (
        <div className="message faint">
          LibraTable can't show {detail.type} values here; use the query editor.
        </div>
      );
  }
}

export function ValuePanel(p: ValuePanelProps) {
  const { detail, error, loading, reload, loadMore } = useKeyDetail(p.target, p.keyName);
  const exists = detail && detail.value.kind !== "missing";
  const setTtl = async () => {
    if (detail && (await askTtl(p, detail))) {
      void reload();
      p.onChanged();
    }
  };
  const remove = async () => {
    if (await deleteKeys(p.target, [p.keyName])) p.onGone(undefined);
  };
  return (
    <div className="detail kv-panel">
      <div className="detail-head kv-panel-head">
        <span className="mono ellipsis grow" title={valueText(p.keyName)}>
          {valueText(p.keyName)}
        </span>
        <button className="btn xs icon ghost" title="Reload" onClick={() => void reload()}>
          <BusyIcon busy={loading} icon={<RefreshCw size={12} />} />
        </button>
        <button className="btn xs icon ghost" aria-label="Close" onClick={p.onClose}>
          <X size={13} />
        </button>
      </div>
      {detail && (
        <div className="kv-meta">
          <span className="badge xs">{detail.type}</span>
          {detail.size !== null && <span className="muted">{formatCount(detail.size)}</span>}
          {detail.encoding && <span className="faint">{detail.encoding}</span>}
          <span className="spacer" />
          <span className="muted" title="Time to live">
            <Clock size={12} className="inline-icon" /> {ttlText(detail.ttl)}
          </span>
          {exists && p.canEdit && (
            <>
              <button className="btn xs" onClick={() => void setTtl()}>
                Set TTL
              </button>
              <button className="btn xs icon" title="Rename" onClick={() => void askRename(p)}>
                <Pencil size={12} />
              </button>
              <button
                className="btn xs icon danger-outline"
                title="Delete key"
                onClick={() => void remove()}
              >
                <Trash2 size={12} />
              </button>
            </>
          )}
        </div>
      )}
      <div className="detail-body kv-body">
        <ErrorCallout error={error} />
        {detail && <Body {...p} detail={detail} reload={() => void reload()} />}
        {detail?.next && (
          <button className="btn sm" disabled={loading} onClick={() => void loadMore()}>
            Load more
          </button>
        )}
      </div>
    </div>
  );
}
