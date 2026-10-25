// Fake handlers for schema objects, the sidebar's recent/pinned items and running SQL.

import { type SchemaObject, type SidebarState, type StatementResult } from "@/types";

/** Args as a command received them. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Args = Record<string, any>;

const object = (o: Partial<SchemaObject> & Pick<SchemaObject, "kind" | "name">): SchemaObject => ({
  arguments: null,
  table: null,
  detail: null,
  labels: [],
  ...o,
});

export const SCHEMA_OBJECTS: SchemaObject[] = [
  object({
    kind: "function",
    name: "add",
    arguments: "a integer, b integer",
    detail: "returns integer",
  }),
  object({ kind: "materialized_view", name: "totals" }),
  object({ kind: "trigger", name: "audit", table: "orders", detail: "BEFORE INSERT · row" }),
  object({ kind: "sequence", name: "ids" }),
  object({ kind: "type", name: "mood", detail: "enum", labels: ["sad", "ok"] }),
];

export function functionDdl(name: string) {
  return `CREATE OR REPLACE FUNCTION public.${name}(a integer, b integer)\n RETURNS integer;`;
}

/** Statements split on `;` with their offsets, like `db::split` (no strings handling). */
export function splitStatements(sql: string) {
  const out: { text: string; from: number; to: number }[] = [];
  let start = 0;
  for (let i = 0; i <= sql.length; i++) {
    if (i < sql.length && sql[i] !== ";") continue;
    const raw = sql.slice(start, i);
    const lead = raw.length - raw.trimStart().length;
    const text = raw.trim();
    if (text) out.push({ text, from: start + lead, to: start + lead + text.length });
    start = i + 1;
  }
  return out;
}

export function okResult(sql: string, extra: Partial<StatementResult> = {}): StatementResult {
  return {
    sql,
    columns: [{ name: "?column?", typeName: "INT4" }],
    rows: [[1]],
    totalRows: 1,
    truncated: false,
    rowsAffected: null,
    durationMs: 2,
    error: null,
    ...extra,
  };
}

export interface ObjectsState {
  schemaObjects: SchemaObject[];
  sidebar: SidebarState;
  /** Answers run_query; the default runs each statement fine. */
  runQuery: (args: Args) => StatementResult[];
}

export function objectHandlers(s: ObjectsState): Record<string, (args: Args) => unknown> {
  return {
    list_schema_objects: () => s.schemaObjects,
    object_definition: (a) => ({
      ddl: functionDdl(a.object.name),
      editScript: `-- edit\n${functionDdl(a.object.name)}`,
    }),
    sequence_info: () => ({
      dataType: "bigint",
      startValue: "1",
      minValue: "1",
      maxValue: "9223372036854775807",
      increment: "1",
      cycle: false,
      cache: "1",
      lastValue: "41",
    }),
    get_sidebar_state: () => s.sidebar,
    save_sidebar_state: (a) => {
      s.sidebar = a.sidebar;
      return null;
    },
    split_statements: (a) => splitStatements(a.sql),
    run_query: (a) => ({ results: s.runQuery(a), cancelled: false }),
  };
}
