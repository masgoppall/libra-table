// Which actions a driver offers, from its capabilities (src-tauri/src/drivers.rs).
// Actions not listed here work for every driver.

import { type ActionId } from "@/lib/actions";
import { type Capabilities } from "@/types";

const NEEDS: Partial<Record<ActionId, (c: Capabilities) => boolean>> = {
  "new-query": (c) => c.query,
  "run-current": (c) => c.query,
  "run-all": (c) => c.query,
  history: (c) => c.query,
  "new-table": (c) => c.createTable,
  "new-database": (c) => c.createDatabase,
  "open-structure": (c) => c.structure !== "none",
  "switch-database": (c) => c.switchDatabase,
  explain: (c) => c.explain,
  "format-sql": (c) => c.sqlDialect !== null,
  "trim-whitespace": (c) => c.query,
  "ask-ai": (c) => c.sqlDialect !== null,
  "process-list": (c) => c.processes,
  users: (c) => c.users,
  backup: (c) => c.backup,
  restore: (c) => c.backup && c.processes,
  "command-line": (c) => c.backup,
  "import-data": (c) => c.import,
  "import-new-table": (c) => c.import && c.createTable,
  "import-sql": (c) => c.import,
  "open-diagram": (c) => c.diagram,
  "open-metrics": (c) => c.metrics,
  "compare-schema": (c) => c.structure !== "none" && c.sqlDialect !== null,
};

export function actionAvailable(caps: Capabilities, id: ActionId): boolean {
  return NEEDS[id]?.(caps) ?? true;
}

/** Wraps action handlers so the ones the driver lacks report "not handled". */
export function onlyAvailable<T extends Partial<Record<ActionId, () => unknown>>>(
  caps: Capabilities,
  handlers: T,
): T {
  const out: Partial<Record<ActionId, () => unknown>> = {};
  for (const [id, run] of Object.entries(handlers) as [ActionId, () => unknown][]) {
    out[id] = () => (actionAvailable(caps, id) ? run() : false);
  }
  return out as T;
}
