// Which statement runs, and which result to show first.

import { type RunResponse, type Statement } from "@/types";

/** The statement containing the cursor, else the last one starting before it. */
export function statementAt(stmts: Statement[], cursor: number): Statement | undefined {
  const containing = stmts.find((s) => cursor >= s.from && cursor <= s.to + 1);
  return containing ?? stmts.filter((s) => s.from <= cursor).pop() ?? stmts[0];
}

/** The first error, else the last result with rows, else the last statement. */
export function initialResult(res: RunResponse): number {
  const firstError = res.results.findIndex((r) => r.error);
  if (firstError >= 0) return firstError;
  const withRows = res.results.map((r, i) => (r.columns.length ? i : -1)).filter((i) => i >= 0);
  return withRows.pop() ?? Math.max(0, res.results.length - 1);
}
