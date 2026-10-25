// One tab per result, with the append-mode toggle and Clear on the right.

import { ListPlus, Trash2, X } from "lucide-react";
import { cx } from "@/lib/cx";
import { formatCount } from "@/lib/format";
import { type ResultItem } from "./results";
import { type QueryRunner } from "./useQueryRunner";

function label(item: ResultItem, i: number, append: boolean): string {
  const r = item.result;
  const base = r.error
    ? "Error"
    : item.explain
      ? "Plan"
      : r.columns.length
        ? `Result ${i + 1}`
        : `Statement ${i + 1}`;
  return append ? `#${item.run} ${base}` : base;
}

export function ResultTabs({ runner }: { runner: QueryRunner }) {
  const showRuns = runner.append || new Set(runner.items.map((i) => i.run)).size > 1;
  return (
    <div className="result-tabs">
      {runner.items.map((item, i) => {
        const r = item.result;
        const hasRows = !r.error && r.columns.length > 0 && !item.explain;
        const classes = cx("result-tab", i === runner.active && "active", !!r.error && "error");
        return (
          <div key={item.key} className={classes} onClick={() => runner.setActive(i)} title={r.sql}>
            {label(item, i, showRuns)}
            {hasRows && <span className="result-count">{formatCount(r.totalRows)}</span>}
            {runner.append && (
              <span
                className="result-close"
                title="Close this result"
                onClick={(e) => {
                  e.stopPropagation();
                  runner.closeResult(i);
                }}
              >
                <X size={11} />
              </span>
            )}
          </div>
        );
      })}
      {runner.cancelled && <div className="result-tab error">Cancelled</div>}
      <span className="spacer" />
      <div className="result-tools">
        <button
          className={cx("btn xs ghost", runner.append && "on")}
          aria-pressed={runner.append}
          onClick={() => runner.setAppend(!runner.append)}
          title="Append mode: new runs add result tabs instead of replacing them"
        >
          <ListPlus size={12} /> Append
        </button>
        {runner.items.length > 0 && (
          <button className="btn xs ghost" onClick={runner.clear} title="Clear all results">
            <Trash2 size={12} /> Clear
          </button>
        )}
      </div>
    </div>
  );
}
