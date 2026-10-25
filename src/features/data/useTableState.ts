// Column order and widths, filters and sort of a table, saved per table (STO-4).

import { useEffect, useState } from "react";
import { api } from "@/api";
import { type TableState } from "@/types";

const EMPTY: TableState = { columnOrder: [], columnWidths: {}, filters: [], sort: [] };

export function useTableState(key: string, onLoad: (s: TableState) => void) {
  const [state, setState] = useState<TableState | null>(null);

  useEffect(() => {
    api
      .getTableState(key)
      .then((s) => {
        const loaded = s ?? EMPTY;
        setState(loaded);
        onLoad(loaded);
      })
      .catch(() => setState(EMPTY));
  }, [key]);

  const save = (patch: Partial<TableState>) =>
    setState((prev) => {
      const next = { ...(prev ?? EMPTY), ...patch };
      api.saveTableState(key, next).catch(() => undefined);
      return next;
    });

  return { state, save };
}
