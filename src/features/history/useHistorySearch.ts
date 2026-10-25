// History search (QRY-6): text, connection and date range, paged.

import { useCallback, useState } from "react";
import { api } from "@/api";
import { useDebouncedEffect } from "@/hooks/useDebouncedEffect";
import { showError } from "@/services/errors";
import { type HistoryEntry } from "@/types";

const PAGE = 200;
const SEARCH_DELAY_MS = 200;

export type HistoryScope = "this" | "all";

export interface HistoryFilters {
  text: string;
  scope: HistoryScope;
  /** yyyy-mm-dd from the date inputs, or "". */
  from: string;
  to: string;
}

const dayStart = (d: string) => (d ? new Date(`${d}T00:00:00`).getTime() : null);
const dayEnd = (d: string) => (d ? new Date(`${d}T23:59:59.999`).getTime() : null);

export function useHistorySearch(connectionId: string) {
  const [filters, setFilters] = useState<HistoryFilters>({
    text: "",
    scope: "this",
    from: "",
    to: "",
  });
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [more, setMore] = useState(false);

  const load = useCallback(
    async (offset = 0) => {
      setLoading(true);
      try {
        const res = await api.searchHistory({
          text: filters.text,
          connectionId: filters.scope === "this" ? connectionId : null,
          from: dayStart(filters.from),
          to: dayEnd(filters.to),
          limit: PAGE,
          offset,
        });
        setItems((prev) => (offset ? [...prev, ...res] : res));
        setMore(res.length === PAGE);
      } catch (e) {
        showError(e);
      } finally {
        setLoading(false);
      }
    },
    [filters, connectionId],
  );

  useDebouncedEffect(() => void load(0), [load], SEARCH_DELAY_MS);

  const setFilter = (patch: Partial<HistoryFilters>) => setFilters((f) => ({ ...f, ...patch }));
  const remove = (id: number) => setItems((xs) => xs.filter((x) => x.id !== id));
  const loadMore = () => void load(items.length);

  return { filters, setFilter, items, loading, more, load, loadMore, remove };
}
