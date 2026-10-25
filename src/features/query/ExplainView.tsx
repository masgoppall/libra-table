// An EXPLAIN result as a plan: Postgres JSON and SQLite QUERY PLAN as an indented
// tree with costs and rows; MySQL's tabular plan stays a grid (`fallback`).

import { type ReactNode } from "react";
import { formatDuration } from "@/lib/format";
import { flattenPlan, pgPlanTree, type PlanTree, sqlitePlanTree } from "@/lib/explainPlan";
import { type PlanNode, type StatementResult } from "@/types";
import { type ExplainSpec } from "./results";

function planOf(spec: ExplainSpec, r: StatementResult): PlanTree | string | null {
  if (spec.driver === "postgres") {
    try {
      return pgPlanTree(r.rows[0]?.[0]);
    } catch {
      return null;
    }
  }
  if (spec.driver === "sqlite") {
    return sqlitePlanTree(
      r.columns.map((c) => c.name),
      r.rows,
    );
  }
  // MySQL EXPLAIN ANALYZE answers with one text cell: the tree, already indented.
  const text = r.rows[0]?.[0];
  return spec.analyze && r.columns.length === 1 && typeof text === "string" ? text : null;
}

function Measures({ n }: { n: PlanNode }) {
  return (
    <span className="plan-measures mono">
      {n.cost && <span title="Estimated startup..total cost">cost {n.cost}</span>}
      {n.rows !== undefined && <span title="Estimated rows">rows {n.rows}</span>}
      {n.actualRows !== undefined && (
        <span className="plan-actual" title="Actual rows × loops, total time">
          actual {n.actualRows}
          {n.loops && n.loops > 1 ? ` ×${n.loops}` : ""}
          {n.actualMs !== undefined ? ` · ${n.actualMs} ms` : ""}
        </span>
      )}
    </span>
  );
}

function Tree({ tree }: { tree: PlanTree }) {
  const timing = [
    tree.planningMs !== undefined && `Planning ${formatDuration(tree.planningMs)}`,
    tree.executionMs !== undefined && `Execution ${formatDuration(tree.executionMs)}`,
  ].filter(Boolean);
  return (
    <div className="plan selectable">
      {timing.length > 0 && <div className="plan-timing faint">{timing.join(" · ")}</div>}
      {flattenPlan(tree.roots).map(({ node, depth }, i) => (
        <div key={i} className="plan-row" style={{ paddingLeft: 12 + depth * 18 }}>
          <div className="plan-head">
            <span className="plan-arrow">{depth > 0 ? "↳" : "●"}</span>
            <strong>{node.label}</strong>
            <span className="spacer" />
            <Measures n={node} />
          </div>
          {node.details.map((d) => (
            <div key={d} className="plan-detail mono">
              {d}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

export function ExplainView(props: {
  spec: ExplainSpec;
  result: StatementResult;
  fallback: ReactNode;
}) {
  const plan = planOf(props.spec, props.result);
  if (plan === null) return <>{props.fallback}</>;
  if (typeof plan === "string") return <pre className="plan plan-text mono selectable">{plan}</pre>;
  return <Tree tree={plan} />;
}
