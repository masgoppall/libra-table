// Results of a run: one tab per statement (kept across runs in append mode), rows in a
// grid, the affected count, or an EXPLAIN plan.

import { Download, Play } from "lucide-react";
import { useEffect, useState } from "react";
import { PaddedError, ErrorCallout } from "@/components/ErrorCallout";
import { DataGrid } from "@/components/grid/DataGrid";
import { Spinner } from "@/components/Spinner";
import { EmptyState } from "@/components/StateView";
import { shortcutFor, withShortcut } from "@/lib/actions";
import { copyText } from "@/lib/clipboard";
import { copyRows } from "@/lib/copyRows";
import { formatCount, formatDuration, plural } from "@/lib/format";
import { toastSuccess } from "@/services/errors";
import { type Dialect, type StatementResult } from "@/types";
import { ExplainView } from "./ExplainView";
import { ResultTabs } from "./ResultTabs";
import { type QueryRunner } from "./useQueryRunner";

function rowSummary(r: StatementResult, rowLimit: number): string {
  if (!r.truncated) return plural(r.totalRows, "row");
  const shown = formatCount(r.rows.length);
  return `First ${shown} of ${formatCount(r.totalRows)} rows (row limit ${formatCount(rowLimit)})`;
}

function ResultBody(props: {
  result: StatementResult;
  selected: Set<number>;
  onSelect: (s: Set<number>) => void;
  onCopy: () => void;
  widths: Record<string, number>;
  onWidths: (w: Record<string, number>) => void;
}) {
  const r = props.result;
  if (r.error) {
    return (
      <div className="pad col">
        <ErrorCallout error={r.error} />
        <pre className="sql-preview mono selectable">{r.sql}</pre>
      </div>
    );
  }
  if (!r.columns.length) {
    const affected =
      r.rowsAffected !== null ? `${plural(r.rowsAffected, "row")} affected.` : "Done.";
    return (
      <div className="message done">
        {affected}
        <pre className="sql-preview mono selectable">{r.sql}</pre>
      </div>
    );
  }
  return (
    <div className="grid-wrap">
      <DataGrid
        columns={r.columns.map((c) => ({ name: c.name, typeName: c.typeName }))}
        rowCount={r.rows.length}
        getCell={(row, col) => r.rows[row]?.[col] ?? null}
        widths={props.widths}
        onWidths={props.onWidths}
        selected={props.selected}
        onSelect={(s) => props.onSelect(s)}
        onCopy={props.onCopy}
        emptyText="The query returned no rows."
      />
    </div>
  );
}

function ResultStatus(props: {
  result: StatementResult;
  rowLimit: number;
  selected: number;
  onExport: () => void;
}) {
  const r = props.result;
  const hasRows = r.columns.length > 0 && !r.error;
  return (
    <div className="statusbar">
      {hasRows && <span className="status-strong">{rowSummary(r, props.rowLimit)}</span>}
      <span className="faint">{formatDuration(r.durationMs)}</span>
      {props.selected > 0 && <span className="status-pill">{props.selected} selected</span>}
      <span className="spacer" />
      {hasRows && (
        <button
          className="btn xs"
          onClick={props.onExport}
          title={withShortcut("Export result", "export")}
        >
          <Download size={12} /> Export result
        </button>
      )}
    </div>
  );
}

function EmptyResults() {
  const [runOne, runAll, save] = (["run-current", "run-all", "save"] as const).map((a) => (
    <kbd key={a}>{shortcutFor(a)}</kbd>
  ));
  return (
    <EmptyState
      art="query-results"
      icon={<Play size={18} />}
      title="Results appear here"
      hint={
        <>
          {runOne} runs the statement under the cursor · {runAll} runs everything · {save} saves the
          query
        </>
      }
    />
  );
}

export function ResultsPane(props: {
  runner: QueryRunner;
  driver: Dialect;
  rowLimit: number;
  onExport: (sql: string) => void;
}) {
  const { running, items, error, item, result, cancel } = props.runner;
  const [selected, setSelected] = useState<Set<number>>(new Set());
  // Column widths are shared by the results of the tab (same names, same widths).
  const [widths, setWidths] = useState<Record<string, number>>({});
  useEffect(() => setSelected(new Set()), [item?.key]);

  const copySelected = async () => {
    if (!result || selected.size === 0) return;
    const picked = [...selected].sort((a, b) => a - b);
    const rows = picked.flatMap((i) => (result.rows[i] ? [result.rows[i]] : []));
    const names = result.columns.map((c) => c.name);
    await copyText(copyRows("tsv", names, rows, { driver: props.driver }));
    toastSuccess(`Copied ${plural(selected.size, "row")}.`);
  };

  const body = result && (
    <ResultBody
      key={item?.key}
      result={result}
      selected={selected}
      onSelect={setSelected}
      onCopy={() => void copySelected()}
      widths={widths}
      onWidths={setWidths}
    />
  );
  return (
    <div className="results">
      <PaddedError error={error} />
      {running && (
        <div className="running">
          <Spinner /> Running…
          <button className="btn xs" onClick={cancel}>
            Cancel
          </button>
        </div>
      )}
      {!running && (items.length > 0 || props.runner.append) && (
        <ResultTabs runner={props.runner} />
      )}
      {!running && item && result && (
        <>
          {item.explain && !result.error ? (
            <ExplainView spec={item.explain} result={result} fallback={body} />
          ) : (
            body
          )}
          <ResultStatus
            result={result}
            rowLimit={props.rowLimit}
            selected={selected.size}
            onExport={() => props.onExport(result.sql)}
          />
        </>
      )}
      {!running && !items.length && !error && <EmptyResults />}
    </div>
  );
}
