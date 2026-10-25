// The process list of a connection: loading, auto-refresh while the tab is visible,
// search and the "hide idle" filter.

import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "@/api";
import { type AppError, type ProcessInfo } from "@/types";
import { filterProcesses, type RefreshInterval } from "./processModel";

export function useProcessList(connectionId: string, visible: boolean) {
  const [all, setAll] = useState<ProcessInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [loadedAt, setLoadedAt] = useState<number | null>(null);
  const [interval, setRefresh] = useState<RefreshInterval>(0);
  const [text, setText] = useState("");
  const [hideIdle, setHideIdle] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setAll(await api.listProcesses(connectionId));
      setError(null);
      setLoadedAt(Date.now());
    } catch (e) {
      setError(e as AppError);
    } finally {
      setLoading(false);
    }
  }, [connectionId]);

  // Load when the tab comes into view, then every `interval` while it stays there.
  useEffect(() => {
    if (!visible) return;
    void load();
    if (!interval) return;
    const timer = window.setInterval(() => void load(), interval);
    return () => window.clearInterval(timer);
  }, [visible, interval, load]);

  const rows = useMemo(() => filterProcesses(all, text, hideIdle), [all, text, hideIdle]);

  return {
    rows,
    total: all.length,
    loading,
    error,
    loadedAt,
    load,
    interval,
    setRefresh,
    text,
    setText,
    hideIdle,
    setHideIdle,
  };
}

export type ProcessList = ReturnType<typeof useProcessList>;
