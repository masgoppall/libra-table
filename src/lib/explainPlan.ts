// EXPLAIN for each driver, and plans turned into trees: Postgres from FORMAT JSON,
// SQLite from EXPLAIN QUERY PLAN rows (id, parent, notused, detail). MySQL plans are
// shown as the table (or text tree, with ANALYZE) the server returns.

import { type Cell, type Dialect, type PlanNode } from "@/types";

export interface PlanTree {
  roots: PlanNode[];
  planningMs?: number;
  executionMs?: number;
}

/** ANALYZE runs the statement; SQLite has no such option. */
export const canAnalyze = (driver: Dialect) => driver !== "sqlite";

/** The EXPLAIN statement for `statement` (a trailing `;` is dropped). */
export function explainSql(driver: Dialect, statement: string, analyze: boolean): string {
  const st = statement.trim().replace(/;+\s*$/, "");
  switch (driver) {
    case "postgres":
      return `EXPLAIN (FORMAT JSON${analyze ? ", ANALYZE" : ""}) ${st}`;
    case "mysql":
      return `${analyze ? "EXPLAIN ANALYZE" : "EXPLAIN"} ${st}`;
    case "sqlite":
      return `EXPLAIN QUERY PLAN ${st}`;
  }
}

type Json = Record<string, unknown>;

const isObject = (v: unknown): v is Json => typeof v === "object" && v !== null;
const num = (v: unknown) => (typeof v === "number" ? v : undefined);
const str = (v: unknown) => (typeof v === "string" ? v : undefined);

/** Conditions and other facts worth a line under the node. */
const DETAIL_KEYS = [
  "Hash Cond",
  "Merge Cond",
  "Index Cond",
  "Recheck Cond",
  "Join Filter",
  "Filter",
  "Rows Removed by Filter",
  "Sort Key",
  "Group Key",
  "Sort Method",
  "Cache Key",
];

function pgLabel(p: Json): string {
  const type = str(p["Node Type"]) ?? "?";
  const qualifier = str(p["Join Type"]) ?? str(p["Strategy"]) ?? str(p["Scan Direction"]);
  return qualifier && qualifier !== "Forward" && qualifier !== "Plain"
    ? `${type} (${qualifier})`
    : type;
}

function pgDetails(p: Json): string[] {
  const out: string[] = [];
  const rel = str(p["Relation Name"]);
  if (rel) {
    const schema = str(p["Schema"]);
    const alias = str(p["Alias"]);
    const name = schema ? `${schema}.${rel}` : rel;
    out.push(`on ${name}${alias && alias !== rel ? ` ${alias}` : ""}`);
  }
  const index = str(p["Index Name"]);
  if (index) out.push(`using ${index}`);
  const cte = str(p["CTE Name"]);
  if (cte) out.push(`CTE ${cte}`);
  for (const key of DETAIL_KEYS) {
    const v = p[key];
    if (v === undefined || v === null || v === 0) continue;
    out.push(`${key}: ${Array.isArray(v) ? v.join(", ") : String(v)}`);
  }
  return out;
}

function pgNode(p: Json): PlanNode {
  const startup = num(p["Startup Cost"]);
  const total = num(p["Total Cost"]);
  const children = Array.isArray(p["Plans"]) ? p["Plans"].filter(isObject).map(pgNode) : [];
  return {
    label: pgLabel(p),
    details: pgDetails(p),
    cost: startup !== undefined && total !== undefined ? `${startup}..${total}` : undefined,
    rows: num(p["Plan Rows"]),
    actualRows: num(p["Actual Rows"]),
    actualMs: num(p["Actual Total Time"]),
    loops: num(p["Actual Loops"]),
    children,
  };
}

/** The tree of `EXPLAIN (FORMAT JSON)`; its one cell is JSON text (or already parsed). */
export function pgPlanTree(cell: unknown): PlanTree {
  const parsed: unknown = typeof cell === "string" ? JSON.parse(cell) : cell;
  const first: unknown = Array.isArray(parsed) ? parsed[0] : parsed;
  if (!isObject(first) || !isObject(first["Plan"])) throw new Error("Not a Postgres JSON plan.");
  return {
    roots: [pgNode(first["Plan"])],
    planningMs: num(first["Planning Time"]),
    executionMs: num(first["Execution Time"]),
  };
}

/** The tree of `EXPLAIN QUERY PLAN` rows; a parent of 0 is a root. */
export function sqlitePlanTree(columns: string[], rows: Cell[][]): PlanTree {
  const col = (name: string) => columns.indexOf(name);
  const [idAt, parentAt, detailAt] = [col("id"), col("parent"), col("detail")];
  const nodes = new Map<number, PlanNode>();
  const roots: PlanNode[] = [];
  for (const r of rows) {
    const node: PlanNode = { label: String(r[detailAt] ?? ""), details: [], children: [] };
    nodes.set(Number(r[idAt]), node);
    const parent = nodes.get(Number(r[parentAt]));
    (parent ? parent.children : roots).push(node);
  }
  return { roots };
}

/** Nodes depth first with their depth, for an indented list. */
export function flattenPlan(roots: PlanNode[]): { node: PlanNode; depth: number }[] {
  const out: { node: PlanNode; depth: number }[] = [];
  const walk = (n: PlanNode, depth: number) => {
    out.push({ node: n, depth });
    n.children.forEach((c) => walk(c, depth + 1));
  };
  roots.forEach((r) => walk(r, 0));
  return out;
}
