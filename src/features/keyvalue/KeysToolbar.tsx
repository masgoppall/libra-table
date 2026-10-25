// Above the key list: the SCAN pattern (applied with Enter), the type filter, refresh,
// new key and delete.

import { Plus, RefreshCw, Search, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { BusyIcon } from "@/components/Spinner";
import { withShortcut } from "@/lib/actions";
import { type KeyFilter } from "./useKeyList";
import { KEY_TYPES } from "./types";

export interface KeysToolbarProps {
  filter: KeyFilter;
  onFilter: (f: KeyFilter) => void;
  loading: boolean;
  canEdit: boolean;
  selectedCount: number;
  onReload: () => void;
  onNewKey: () => void;
  onDelete: () => void;
}

export function KeysToolbar(p: KeysToolbarProps) {
  const [pattern, setPattern] = useState(p.filter.pattern);
  useEffect(() => setPattern(p.filter.pattern), [p.filter.pattern]);
  const apply = () => p.onFilter({ ...p.filter, pattern });
  return (
    <div className="toolbar kv-toolbar">
      <div className="search-field kv-search">
        <Search size={13} className="faint" />
        <input
          className="input sm"
          placeholder="Pattern, e.g. user:* (Enter to search)"
          aria-label="Key pattern"
          value={pattern}
          onChange={(e) => setPattern(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") apply();
            if (e.key === "Escape") {
              setPattern("");
              p.onFilter({ ...p.filter, pattern: "" });
            }
          }}
        />
      </div>
      <select
        className="select sm"
        aria-label="Key type"
        value={p.filter.keyType}
        onChange={(e) => p.onFilter({ ...p.filter, keyType: e.target.value })}
      >
        <option value="">All types</option>
        {KEY_TYPES.map((t) => (
          <option key={t} value={t}>
            {t}
          </option>
        ))}
      </select>
      <button
        className="btn sm icon ghost"
        title={withShortcut("Refresh", "refresh")}
        onClick={p.onReload}
      >
        <BusyIcon busy={p.loading} icon={<RefreshCw size={13} />} />
      </button>
      <span className="spacer" />
      {p.canEdit && (
        <>
          <button
            className="btn sm"
            disabled={p.selectedCount === 0}
            onClick={p.onDelete}
            title="Delete the selected keys"
          >
            <Trash2 size={13} /> Delete
          </button>
          <button className="btn sm primary" onClick={p.onNewKey}>
            <Plus size={13} /> New key
          </button>
        </>
      )}
    </div>
  );
}
