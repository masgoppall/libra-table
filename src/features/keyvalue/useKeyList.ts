// One page of keys (SCAN behind the scenes): pattern, type filter, page and page size.
// Responses of older requests are dropped.

import { useCallback, useEffect, useRef, useState } from "react";
import { toAppError } from "@/api";
import { type AppError, type Db } from "@/types";
import { redisApi } from "./api";
import { type ScanPage } from "./types";

export interface KeyFilter {
  pattern: string;
  keyType: string;
}

export function useKeyList(id: string, db: Db, refreshKey: number, initialPageSize: number) {
  const [filter, setFilterState] = useState<KeyFilter>({ pattern: "", keyType: "" });
  const [page, setPage] = useState(0);
  const [pageSize, setPageSize] = useState(initialPageSize);
  const [result, setResult] = useState<ScanPage | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const seq = useRef(0);

  const load = useCallback(async () => {
    const mine = ++seq.current;
    setLoading(true);
    try {
      const r = await redisApi.scan(id, db, { ...filter, page, pageSize });
      if (mine !== seq.current) return;
      setResult(r);
      setError(null);
    } catch (e) {
      if (mine === seq.current) setError(toAppError(e));
    } finally {
      if (mine === seq.current) setLoading(false);
    }
  }, [id, db, filter, page, pageSize]);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  const setFilter = (f: KeyFilter) => {
    setFilterState(f);
    setPage(0);
  };
  const changePageSize = (n: number) => {
    setPageSize(n);
    setPage(0);
  };

  return {
    filter,
    setFilter,
    page,
    setPage,
    pageSize,
    setPageSize: changePageSize,
    result,
    loading,
    error,
    reload: load,
  };
}

export type KeyList = ReturnType<typeof useKeyList>;
