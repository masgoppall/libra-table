// The "Search and Replace Within Table" bar above the grid: matches in the loaded rows
// are highlighted; ↵ / ⇧↵ step through them; Replace changes the current match and moves
// on, Replace All (⌘↵) every match. Replacements are ordinary pending edits.

import { ChevronDown, ChevronUp, Replace, Search, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { cx } from "@/lib/cx";
import { type FindOptions } from "./findReplace";

export interface FindBarProps {
  options: FindOptions;
  onOptions: (patch: Partial<FindOptions>) => void;
  replacement: string;
  onReplacement: (v: string) => void;
  showReplace: boolean;
  onShowReplace: (v: boolean) => void;
  columns: { index: number; name: string }[];
  count: number;
  /** Position of the current match, -1 when none is current. */
  current: number;
  error: string | null;
  canReplace: boolean;
  focusSignal: number;
  onStep: (back: boolean) => void;
  onReplace: () => void;
  onReplaceAll: () => void;
  onClose: () => void;
}

function Toggle(props: { on: boolean; label: string; title: string; onClick: () => void }) {
  return (
    <button
      className={cx("btn sm find-toggle", props.on && "on")}
      aria-pressed={props.on}
      title={props.title}
      onClick={props.onClick}
    >
      {props.label}
    </button>
  );
}

function status(p: FindBarProps): string {
  if (p.error) return p.error;
  if (!p.options.query) return "";
  if (!p.count) return "No results";
  return p.current >= 0 ? `${p.current + 1} of ${p.count}` : `${p.count} matches`;
}

export function FindBar(p: FindBarProps) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, [p.focusSignal]);
  const o = p.options;
  const onSearchKey = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      p.onStep(e.shiftKey);
    } else if (e.key === "Escape") p.onClose();
  };
  const onReplaceKey = (e: React.KeyboardEvent) => {
    if (e.key !== "Enter") return e.key === "Escape" && p.onClose();
    e.preventDefault();
    if (e.metaKey || e.ctrlKey) p.onReplaceAll();
    else p.onReplace();
  };
  return (
    <div className="filters find-bar" role="search">
      <div className="filter-row">
        <Search size={13} className="faint" />
        <input
          ref={input}
          className={cx("input value", p.error && "invalid")}
          placeholder="Search in the loaded rows"
          aria-label="Search within table"
          value={o.query}
          onChange={(e) => p.onOptions({ query: e.target.value })}
          onKeyDown={onSearchKey}
        />
        <select
          className="select col-select"
          aria-label="Search in column"
          value={o.column ?? ""}
          onChange={(e) =>
            p.onOptions({ column: e.target.value === "" ? null : Number(e.target.value) })
          }
        >
          <option value="">All columns</option>
          {p.columns.map((c) => (
            <option key={c.index} value={c.index}>
              {c.name}
            </option>
          ))}
        </select>
        <Toggle
          on={o.matchCase}
          label="Aa"
          title="Match Case"
          onClick={() => p.onOptions({ matchCase: !o.matchCase })}
        />
        <Toggle
          on={o.wholeWord}
          label="W"
          title="Match Whole Word"
          onClick={() => p.onOptions({ wholeWord: !o.wholeWord })}
        />
        <Toggle
          on={o.regex}
          label=".*"
          title="Use Regular Expression"
          onClick={() => p.onOptions({ regex: !o.regex })}
        />
        <span className={cx("find-status small", p.error ? "danger-text" : "faint")}>
          {status(p)}
        </span>
        <button className="btn sm icon" title="Previous (⇧↵)" onClick={() => p.onStep(true)}>
          <ChevronUp size={13} />
        </button>
        <button className="btn sm icon" title="Next (↵)" onClick={() => p.onStep(false)}>
          <ChevronDown size={13} />
        </button>
        <button
          className={cx("btn sm", p.showReplace && "on")}
          onClick={() => p.onShowReplace(!p.showReplace)}
        >
          {p.showReplace ? "Hide Replace" : "Show Replace"}
        </button>
        <button className="btn sm icon ghost" title="Close (esc)" onClick={p.onClose}>
          <X size={13} />
        </button>
      </div>
      {p.showReplace && (
        <div className="filter-row">
          <Replace size={13} className="faint" />
          <input
            className="input value"
            placeholder="Replace with"
            aria-label="Replace with"
            value={p.replacement}
            onChange={(e) => p.onReplacement(e.target.value)}
            onKeyDown={onReplaceKey}
          />
          <button
            className="btn sm"
            disabled={!p.canReplace || !p.count}
            title="Replace in the current match, then go to the next one (↵)"
            onClick={p.onReplace}
          >
            Replace
          </button>
          <button
            className="btn sm primary"
            disabled={!p.canReplace || !p.count}
            title="Replace in every loaded row (⌘↵)"
            onClick={p.onReplaceAll}
          >
            Replace All{p.count ? ` (${p.count})` : ""}
          </button>
        </div>
      )}
    </div>
  );
}
