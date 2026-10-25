// The console log while the panel is open: what Rust kept so far, then live events.

import { useEffect, useState } from "react";
import { api } from "@/api";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { type ConsoleEntry } from "@/types";

/** The panel keeps the newest entries only. */
const MAX_SHOWN = 2000;

export function useConsoleLog() {
  const [entries, setEntries] = useState<ConsoleEntry[]>([]);

  useEffect(() => {
    api
      .consoleEntries()
      .then(setEntries)
      .catch(() => setEntries([]));
  }, []);

  useTauriEvent<ConsoleEntry>("console-log", (e) =>
    setEntries((list) => [...list.slice(-(MAX_SHOWN - 1)), e]),
  );

  const clear = async () => {
    await api.clearConsole().catch(() => undefined);
    setEntries([]);
  };

  return { entries, clear };
}

/** Entries containing `query` (case-insensitive), optionally only slow ones. */
export function filterEntries(entries: ConsoleEntry[], query: string, slowOnly: boolean) {
  const q = query.trim().toLowerCase();
  return entries.filter((e) => (!slowOnly || e.slow) && (!q || e.sql.toLowerCase().includes(q)));
}
