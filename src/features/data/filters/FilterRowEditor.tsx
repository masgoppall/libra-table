// One filter: `column · operator · value`, with its own apply, preview, copy, remove.

import { Code2, Copy, X } from "lucide-react";
import { ANY_COLUMN, RAW_SQL } from "@/domain/filters";
import { type FilterRow, type OperatorInfo } from "@/types";

export interface FilterRowEditorProps {
  row: FilterRow;
  columns: string[];
  operators: OperatorInfo[];
  onChange: (patch: Partial<FilterRow>) => void;
  onApply: () => void;
  onApplyAll: () => void;
  onPreview: () => void;
  onDuplicate: () => void;
  onRemove: () => void;
}

function ColumnSelect(p: FilterRowEditorProps) {
  return (
    <select
      className="select sm col-select"
      value={p.row.column}
      onChange={(e) => p.onChange({ column: e.target.value })}
      title="Column"
    >
      <option value={ANY_COLUMN}>Any column</option>
      <option value={RAW_SQL}>Raw SQL</option>
      <option disabled>──────────</option>
      {p.columns.map((n) => (
        <option key={n} value={n}>
          {n}
        </option>
      ))}
    </select>
  );
}

function ValueInputs(p: FilterRowEditorProps) {
  const { row: r } = p;
  // Enter in a value applies every checked filter.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    p.onApplyAll();
  };
  const input = (field: "value" | "value2", placeholder: string, mono = false) => (
    <input
      className={`input sm value${mono ? " mono" : ""}`}
      placeholder={placeholder}
      value={r[field]}
      onChange={(e) => p.onChange({ [field]: e.target.value })}
      onKeyDown={onKeyDown}
    />
  );
  if (r.column === RAW_SQL) {
    return input("value", "WHERE fragment, e.g. status = 'paid' OR total > 100", true);
  }
  const op = p.operators.find((o) => o.id === r.operator);
  if (op?.valueKind === "none") return <span className="spacer" />;
  if (op?.valueKind === "range") {
    return (
      <>
        {input("value", "from")}
        <span className="muted small">and</span>
        {input("value2", "to")}
      </>
    );
  }
  return input("value", op?.placeholder ?? "value");
}

export function FilterRowEditor(p: FilterRowEditorProps) {
  const { row: r } = p;
  return (
    <div className={`filter-row${r.enabled ? "" : " off"}`}>
      <input
        type="checkbox"
        checked={r.enabled}
        title="Include this filter when applying all"
        onChange={(e) => p.onChange({ enabled: e.target.checked })}
      />
      <ColumnSelect {...p} />
      {r.column !== RAW_SQL && (
        <select
          className="select sm op-select"
          value={r.operator}
          onChange={(e) => p.onChange({ operator: e.target.value })}
          title="Operator"
        >
          {p.operators.map((o) => (
            <option key={o.id} value={o.id}>
              {o.label}
            </option>
          ))}
        </select>
      )}
      <ValueInputs {...p} />
      <button className="btn sm" title="Apply only this filter" onClick={p.onApply}>
        Apply
      </button>
      <button className="btn sm icon ghost" title="SQL preview" onClick={p.onPreview}>
        <Code2 size={13} />
      </button>
      <button className="btn sm icon ghost" title="Duplicate this filter" onClick={p.onDuplicate}>
        <Copy size={13} />
      </button>
      <button className="btn sm icon ghost" title="Remove this filter" onClick={p.onRemove}>
        <X size={13} />
      </button>
    </div>
  );
}
