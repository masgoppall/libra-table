// A widget's query result: run again when its SQL, the variables it uses or the board's
// refresh tick change.

import { useEffect, useRef, useState } from "react";
import { api } from "@/api";
import { type AppError, type Db, type MetricVariable, type MetricWidget } from "@/types";
import { type StatementResult } from "@/types";
import { variableNames } from "./boardModel";

export interface WidgetData {
  result: StatementResult | null;
  error: AppError | null;
  loading: boolean;
}

export function useWidgetData(
  connectionId: string,
  db: Db,
  w: MetricWidget,
  variables: MetricVariable[],
  tick: number,
): WidgetData {
  const [result, setResult] = useState<StatementResult | null>(null);
  const [error, setError] = useState<AppError | null>(null);
  const [loading, setLoading] = useState(false);
  const names = variableNames(w.sql);
  // Only the variables this widget uses: other inputs don't re-run it.
  const used = variables.filter((v) => names.includes(v.name));
  const key = JSON.stringify(used);
  const run = useRef(0);

  useEffect(() => {
    if (w.kind === "input" || !w.sql.trim()) return;
    const n = ++run.current;
    setLoading(true);
    api
      .runMetricQuery(connectionId, db, w.sql, used)
      .then((r) => n === run.current && (setResult(r), setError(null)))
      .catch((e) => n === run.current && (setError(e as AppError), setResult(null)))
      .finally(() => n === run.current && setLoading(false));
  }, [connectionId, db, w.sql, w.kind, key, tick]);

  return { result, error, loading };
}
