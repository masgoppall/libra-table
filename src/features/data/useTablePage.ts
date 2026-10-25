// Loads one page of a table, then counts all matching rows in the background.

import { useCallback, useRef, useState } from "react";
import { api } from "@/api";
import { type AppError, type Db, type PageRequest, type PageResult, type RowCount } from "@/types";

export function useTablePage(connectionId: string, db: Db, onLoaded: () => void) {
  const [result, setResult] = useState<PageResult | null>(null);
  const [count, setCount] = useState<RowCount | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  // Only the latest request may update the view.
  const seqRef = useRef(0);

  const load = useCallback(
    async (request: PageRequest) => {
      const seq = ++seqRef.current;
      const current = () => seq === seqRef.current;
      setLoading(true);
      setError(null);
      try {
        const page = await api.fetchPage(connectionId, db, request);
        if (!current()) return;
        setResult(page);
        setCount(null);
        onLoaded();
        api
          .countRows(connectionId, db, request)
          .then((c) => current() && setCount(c))
          .catch(() => undefined);
      } catch (e) {
        if (current()) setError(e as AppError);
      } finally {
        if (current()) setLoading(false);
      }
    },
    [connectionId, db],
  );

  return { result, count, loading, error, load };
}
