// The widgets that aren't charts: one big value, a small table, and an input that sets a
// `{{variable}}` for the other widgets.

import { useEffect, useState } from "react";
import { type MetricWidget, type StatementResult } from "@/types";
import { cellText, formatValue, toNumber } from "../chartScale";

const TABLE_ROWS = 200;
const INPUT_DELAY_MS = 400;

export function Scoreboard({ w, result }: { w: MetricWidget; result: StatementResult }) {
  const names = result.columns.map((c) => c.name);
  const row = result.rows[0];
  const at = Math.max(0, w.options.valueColumns[0] ? names.indexOf(w.options.valueColumns[0]) : 0);
  const cell = row?.[at];
  const n = toNumber(cell);
  const text = n === null ? (row ? cellText(cell) : "–") : formatValue(n, w.options.format);
  return (
    <div className="scoreboard">
      <div className="scoreboard-value" title={row ? cellText(cell) : undefined}>
        {text}
      </div>
      <div className="scoreboard-label">{w.options.label || names[at] || ""}</div>
    </div>
  );
}

export function TableWidget({ result }: { result: StatementResult }) {
  return (
    <div className="metric-table">
      <table>
        <thead>
          <tr>
            {result.columns.map((c, i) => (
              <th key={i}>{c.name}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {result.rows.slice(0, TABLE_ROWS).map((r, i) => (
            <tr key={i}>
              {r.map((c, j) => (
                <td key={j} className={typeof c === "number" ? "num" : c === null ? "null" : ""}>
                  {cellText(c)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {result.totalRows > TABLE_ROWS && (
        <div className="faint small">
          First {TABLE_ROWS} of {result.totalRows} rows
        </div>
      )}
    </div>
  );
}

export function InputWidget(props: {
  w: MetricWidget;
  value: string;
  onChange: (value: string) => void;
}) {
  const { w } = props;
  const [text, setText] = useState(props.value);
  useEffect(() => setText(props.value), [props.value]);
  // Widgets re-run once typing pauses.
  useEffect(() => {
    if (text === props.value) return;
    const t = setTimeout(() => props.onChange(text), INPUT_DELAY_MS);
    return () => clearTimeout(t);
  }, [text]);
  return (
    <div className="metric-input">
      <input
        className="input"
        type={w.options.inputType === "number" ? "number" : "text"}
        aria-label={w.title || w.options.variable}
        value={text}
        placeholder={w.options.defaultValue || "value"}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && props.onChange(text)}
      />
      <div className="faint small mono">{`{{${w.options.variable}}}`}</div>
    </div>
  );
}
