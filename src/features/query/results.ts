// The result tabs of a query tab: replaced by each run, or kept and added to in
// append mode.

import { type Dialect, type RunResponse, type StatementResult } from "@/types";
import { initialResult } from "./statements";

/** A result shown as a plan (EXPLAIN run from the toolbar). */
export interface ExplainSpec {
  driver: Dialect;
  analyze: boolean;
}

export interface ResultItem {
  key: string;
  /** Which run it came from (1-based, per tab). */
  run: number;
  result: StatementResult;
  explain?: ExplainSpec;
}

export interface ResultsState {
  items: ResultItem[];
  active: number;
  runs: number;
  cancelled: boolean;
}

export const NO_RESULTS: ResultsState = { items: [], active: 0, runs: 0, cancelled: false };

/** Adds a run's results; the first result worth seeing of the new run becomes active. */
export function addRun(
  prev: ResultsState,
  res: RunResponse,
  opts: { append: boolean; explain?: ExplainSpec },
): ResultsState {
  const run = prev.runs + 1;
  const items: ResultItem[] = res.results.map((result, i) => ({
    key: `${run}-${i}`,
    run,
    result,
    explain: opts.explain,
  }));
  const kept = opts.append ? prev.items : [];
  return {
    items: [...kept, ...items],
    active: kept.length + initialResult(res),
    runs: run,
    cancelled: res.cancelled,
  };
}

export function closeResult(prev: ResultsState, index: number): ResultsState {
  const items = prev.items.filter((_, i) => i !== index);
  const active = Math.max(
    0,
    Math.min(prev.active > index ? prev.active - 1 : prev.active, items.length - 1),
  );
  return { ...prev, items, active };
}
