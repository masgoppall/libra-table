import { describe, expect, it } from "vitest";
import { type QueryVariable, type VariableFormat } from "@/types";
import { explainSql, flattenPlan, pgPlanTree, sqlitePlanTree } from "./explainPlan";
import {
  findVariables,
  rememberVariables,
  substituteVariables,
  variableNames,
  variablesFor,
} from "./variables";

const v = (name: string, value: string, mode: QueryVariable["mode"] = "auto"): QueryVariable => ({
  name,
  value,
  mode,
});

describe("query variables", () => {
  it("finds :name outside strings, comments and casts", () => {
    const sql = `SELECT a::text, ':nope', "col:x", $$ :body $$ -- :line
      /* :block */ FROM t WHERE id = :id AND b = (:other) AND c = :id`;
    expect(variableNames(sql, "postgres")).toEqual(["id", "other"]);
    expect(findVariables(sql, "postgres")).toHaveLength(3);
  });

  it("leaves array slices, := and parameters alone", () => {
    expect(variableNames("SELECT arr[1:n], arr[lo:hi] FROM t WHERE x = $1", "postgres")).toEqual(
      [],
    );
    expect(variableNames("SET @a := 1; SELECT @a", "mysql")).toEqual([]);
    // MySQL: # comments, backslash escapes, and --x is not a comment.
    expect(variableNames("SELECT 'it\\'s :a' # :b\nFROM t WHERE x = :c", "mysql")).toEqual(["c"]);
    expect(variableNames("SELECT 1 --:x", "mysql")).toEqual(["x"]);
    // Postgres E'' strings take backslash escapes; plain ones don't.
    expect(variableNames("SELECT E'\\' :a', ':b' || :c", "postgres")).toEqual(["c"]);
    expect(variableNames("SELECT :a, `x:y` FROM t", "sqlite")).toEqual(["a"]);
  });

  it("substitutes quoted values", () => {
    const sql = "SELECT * FROM t WHERE name = :name AND id = :id AND n IS :n";
    const out = substituteVariables(sql, "postgres", [
      v("name", "O'Brien"),
      v("id", "42"),
      v("n", "", "null"),
    ]);
    expect(out).toBe("SELECT * FROM t WHERE name = 'O''Brien' AND id = 42 AND n IS NULL");
    // Leading zeros stay text in auto mode; MySQL doubles backslashes.
    expect(substituteVariables("x = :c", "mysql", [v("c", "007\\")])).toBe("x = '007\\\\'");
    expect(substituteVariables("x = :c", "sqlite", [v("c", "1e3", "number")])).toBe("x = 1e3");
    expect(substituteVariables("x IN (:c)", "sqlite", [v("c", "1, 2", "raw")])).toBe("x IN (1, 2)");
    expect(() => substituteVariables("x = :c", "sqlite", [v("c", "1; DROP", "number")])).toThrow(
      ":c is not a number.",
    );
    expect(() => substituteVariables("x = :d", "sqlite", [])).toThrow("No value for :d.");
  });

  it("remembers values per name", () => {
    const old = [v("a", "1"), v("b", "2")];
    expect(variablesFor(["b", "c"], old)).toEqual([v("b", "2"), v("c", "")]);
    expect(rememberVariables(old, [v("b", "3")])).toEqual([v("b", "3"), v("a", "1")]);
  });
});

describe("explain", () => {
  it("wraps the statement per driver", () => {
    expect(explainSql("postgres", "SELECT 1;\n", true)).toBe(
      "EXPLAIN (FORMAT JSON, ANALYZE) SELECT 1",
    );
    expect(explainSql("mysql", "SELECT 1", false)).toBe("EXPLAIN SELECT 1");
    expect(explainSql("sqlite", "SELECT 1", true)).toBe("EXPLAIN QUERY PLAN SELECT 1");
  });

  it("builds the Postgres tree from FORMAT JSON", () => {
    const json = JSON.stringify([
      {
        Plan: {
          "Node Type": "Hash Join",
          "Join Type": "Inner",
          "Startup Cost": 1.5,
          "Total Cost": 30.25,
          "Plan Rows": 10,
          "Actual Rows": 8,
          "Actual Total Time": 0.42,
          "Actual Loops": 1,
          "Hash Cond": "(o.person_id = p.id)",
          Plans: [
            { "Node Type": "Seq Scan", "Relation Name": "orders", Schema: "public", Alias: "o" },
            {
              "Node Type": "Hash",
              Plans: [
                {
                  "Node Type": "Index Scan",
                  "Relation Name": "people",
                  Alias: "people",
                  "Index Name": "people_pkey",
                  "Scan Direction": "Forward",
                },
              ],
            },
          ],
        },
        "Planning Time": 0.1,
        "Execution Time": 0.5,
      },
    ]);
    const tree = pgPlanTree(json);
    expect(tree.executionMs).toBe(0.5);
    const flat = flattenPlan(tree.roots);
    expect(flat.map((f) => [f.depth, f.node.label])).toEqual([
      [0, "Hash Join (Inner)"],
      [1, "Seq Scan"],
      [1, "Hash"],
      [2, "Index Scan"],
    ]);
    const root = tree.roots[0];
    expect(root?.cost).toBe("1.5..30.25");
    expect(root?.details).toEqual(["Hash Cond: (o.person_id = p.id)"]);
    expect(flat[1]?.node.details).toEqual(["on public.orders o"]);
    expect(flat[3]?.node.details).toEqual(["on people", "using people_pkey"]);
    expect(() => pgPlanTree("[]")).toThrow();
  });

  it("builds the SQLite tree from parent ids", () => {
    const cols = ["id", "parent", "notused", "detail"];
    const rows = [
      [2, 0, 0, "SCAN people"],
      [5, 0, 0, "CORRELATED SCALAR SUBQUERY 1"],
      [9, 5, 0, "SEARCH orders USING INDEX o_idx (person_id=?)"],
    ];
    const flat = flattenPlan(sqlitePlanTree(cols, rows).roots);
    expect(flat.map((f) => [f.depth, f.node.label])).toEqual([
      [0, "SCAN people"],
      [0, "CORRELATED SCALAR SUBQUERY 1"],
      [1, "SEARCH orders USING INDEX o_idx (person_id=?)"],
    ]);
  });
});

describe("variable formats", () => {
  it("finds variables written in the chosen format", () => {
    const names = (sql: string, format: VariableFormat, inStrings = false) =>
      variableNames(sql, "postgres", { format, inStrings });
    expect(names("select $id, $$body$$, $1", "dollar")).toEqual(["id"]);
    expect(names("select ${ id }, {{x}}", "dollar-braces")).toEqual(["id"]);
    expect(names("select {{ region }} -- {{no}}", "double-braces")).toEqual(["region"]);
    expect(names("select %a% , '%b%'", "percent")).toEqual(["a"]);
    expect(names("where name like '%' || :q || '%' and x = ':s'", "colon")).toEqual(["q"]);
    expect(names("where x = ':s' -- :c", "colon", true)).toEqual(["s"]);
  });

  it("substitutes them", () => {
    const sql = substituteVariables(
      "select {{n}}",
      "postgres",
      [{ name: "n", value: "5", mode: "auto" }],
      { format: "double-braces" },
    );
    expect(sql).toBe("select 5");
  });
});
