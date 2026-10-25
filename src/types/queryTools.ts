// Query editor tools: variables (`:name`) and EXPLAIN plans.

/** How a variable's value goes into the SQL. */
export type VariableMode = "auto" | "text" | "number" | "null" | "raw";

export interface QueryVariable {
  name: string;
  value: string;
  mode: VariableMode;
}

export interface PlanNode {
  label: string;
  /** Relation, index, join condition, filter… */
  details: string[];
  /** "0.00..35.50" (Postgres). */
  cost?: string;
  rows?: number;
  actualRows?: number;
  actualMs?: number;
  loops?: number;
  children: PlanNode[];
}
