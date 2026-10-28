// Filter rows (FLT-1..FLT-6). Operators come from the driver's capabilities; values are
// sent as bind parameters by Rust.

import { Play, Plus, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { newFilterRow } from "@/domain/filters";
import { shortcutFor } from "@/lib/actions";
import { type FilterPreview, type FilterRow, type OperatorInfo } from "@/types";
import { type ApplyTarget } from "../useDataFilters";
import { FilterRowEditor } from "./FilterRowEditor";

const APPLY_ALL = -1;

export interface FilterBarProps {
  columns: { name: string }[];
  operators: OperatorInfo[];
  rows: FilterRow[];
  defaultOp: string;
  defaultColumn: string;
  /** Changes when ⌘F asks for focus. */
  focusSignal: number;
  onChange: (rows: FilterRow[]) => void;
  onApply: (target: ApplyTarget) => void;
  onRemove: (index: number) => void;
  /** Whether any filter is applied (Clear is offered then too). */
  active: boolean;
  onClear: () => void;
  onPreview: (only: number | null) => Promise<FilterPreview | null>;
}

/** Focuses the last value field (or the first select) when ⌘F asks. */
function useFocusOnSignal(signal: number) {
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!signal) return;
    const t = setTimeout(() => {
      const inputs = root.current?.querySelectorAll<HTMLInputElement>("input.value");
      const last = inputs?.[inputs.length - 1];
      (last ?? root.current?.querySelector<HTMLSelectElement>("select"))?.focus();
    }, 0);
    return () => clearTimeout(t);
  }, [signal]);
  return root;
}

function PreviewBlock({ preview }: { preview: FilterPreview }) {
  return (
    <pre className="sql-preview mono selectable">
      {preview.row !== null && `-- This filter\n${preview.row || "(no condition)"}\n\n`}
      {`-- All checked filters\n${preview.all || "(no condition)"}`}
    </pre>
  );
}

export function FilterBar(props: FilterBarProps) {
  const [preview, setPreview] = useState<FilterPreview | null>(null);
  const root = useFocusOnSignal(props.focusSignal);
  const rows = props.rows;
  const names = props.columns.map((c) => c.name);
  const update = (i: number, patch: Partial<FilterRow>) =>
    props.onChange(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const togglePreview = async (only: number | null) =>
    setPreview(preview ? null : await props.onPreview(only));
  const addRow = () =>
    props.onChange([...rows, newFilterRow(props.defaultColumn, props.defaultOp)]);

  return (
    <div className="filters" ref={root}>
      {rows.map((r, i) => (
        <FilterRowEditor
          key={i}
          row={r}
          columns={names}
          operators={props.operators}
          onChange={(patch) => update(i, patch)}
          onApply={() => props.onApply(i)}
          onApplyAll={() => props.onApply(APPLY_ALL)}
          onPreview={() => void togglePreview(i)}
          onDuplicate={() =>
            props.onChange([...rows.slice(0, i + 1), { ...r }, ...rows.slice(i + 1)])
          }
          onRemove={() => props.onRemove(i)}
        />
      ))}
      <div className="row filter-foot">
        <button className="btn sm ghost" onClick={addRow}>
          <Plus size={13} /> Add filter
        </button>
        <span className="faint small">
          Checked filters combine with AND · use Raw SQL for OR · ↵ applies
        </span>
        <span className="spacer" />
        <button
          className="btn sm"
          onClick={props.onClear}
          disabled={!props.active && !rows.some((r) => r.value || r.value2)}
          title="Remove every filter and show all rows"
        >
          <X size={12} /> Clear
        </button>
        <button
          className="btn sm primary"
          onClick={() => props.onApply(APPLY_ALL)}
          disabled={rows.length === 0}
          title={`Apply all (↵) · toggle with ${shortcutFor("filter")}`}
        >
          <Play size={12} /> Apply all
        </button>
      </div>
      {preview && <PreviewBlock preview={preview} />}
    </div>
  );
}
