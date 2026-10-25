// Runs SQL in a query tab, one run at a time, with safe mode and cancel (QRY-2, QRY-4).
// Results replace the previous ones, or are added to them in append mode.

import { useState } from "react";
import { api } from "@/api";
import { isDdl } from "@/lib/sql";
import { uid } from "@/lib/uid";
import { showError } from "@/services/errors";
import { withSafeMode } from "@/services/safeMode";
import { store } from "@/store";
import { type AppError, type Db, type RunResponse } from "@/types";
import { addRun, closeResult, type ExplainSpec, NO_RESULTS, type ResultsState } from "./results";

export function useQueryRunner(connectionId: string, db: Db) {
  const [running, setRunning] = useState<string | null>(null);
  const [results, setResults] = useState<ResultsState>(NO_RESULTS);
  const [append, setAppend] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  /** The response, or null when nothing ran (cancelled confirmation, error, busy). */
  const run = async (sql: string, explain?: ExplainSpec): Promise<RunResponse | null> => {
    if (!sql.trim() || running) return null;
    const queryId = uid();
    setRunning(queryId);
    setError(null);
    try {
      const name = store.connection(connectionId)?.name ?? "";
      const res = await withSafeMode(name, (confirmed) =>
        api.runQuery(connectionId, db, sql, queryId, confirmed),
      );
      if (!res) return null;
      setResults((prev) => addRun(prev, res, { append, explain }));
      // DDL in the editor changes what the sidebar should show.
      if (res.results.some((r) => isDdl(r.sql) && !r.error)) store.get().refresh(connectionId);
      return res;
    } catch (e) {
      setError(e as AppError);
      return null;
    } finally {
      setRunning(null);
    }
  };

  /** False when nothing is running (so the key can do something else). */
  const cancel = () => {
    if (!running) return false;
    api.cancelQuery(running).catch(showError);
  };

  const item = results.items[results.active];
  return {
    running,
    error,
    items: results.items,
    active: results.active,
    cancelled: results.cancelled,
    setActive: (active: number) => setResults((r) => ({ ...r, active })),
    closeResult: (i: number) => setResults((r) => closeResult(r, i)),
    clear: () => setResults((r) => ({ ...NO_RESULTS, runs: r.runs })),
    append,
    setAppend,
    item,
    result: item?.result,
    run,
    cancel,
  };
}

export type QueryRunner = ReturnType<typeof useQueryRunner>;
