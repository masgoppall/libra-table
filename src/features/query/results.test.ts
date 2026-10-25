import { describe, expect, it } from "vitest";
import { type RunResponse, type StatementResult } from "@/types";
import { addRun, closeResult, NO_RESULTS } from "./results";

const result = (sql: string, extra: Partial<StatementResult> = {}): StatementResult => ({
  sql,
  columns: [{ name: "x", typeName: "INT4" }],
  rows: [[1]],
  totalRows: 1,
  truncated: false,
  rowsAffected: null,
  durationMs: 1,
  error: null,
  ...extra,
});
const run = (...results: StatementResult[]): RunResponse => ({ results, cancelled: false });

describe("result tabs", () => {
  it("replaces results unless appending", () => {
    const one = addRun(NO_RESULTS, run(result("a"), result("b")), { append: false });
    expect(one.items.map((i) => i.result.sql)).toEqual(["a", "b"]);
    expect(one.active).toBe(1);
    const replaced = addRun(one, run(result("c")), { append: false });
    expect(replaced.items.map((i) => i.result.sql)).toEqual(["c"]);
    const appended = addRun(one, run(result("c")), { append: true });
    expect(appended.items.map((i) => [i.run, i.result.sql])).toEqual([
      [1, "a"],
      [1, "b"],
      [2, "c"],
    ]);
    expect(appended.active).toBe(2);
    expect(new Set(appended.items.map((i) => i.key)).size).toBe(3);
  });

  it("shows the first error of the new run and closes tabs", () => {
    const err = { kind: "query" as const, layer: "database" as const, message: "boom" };
    const one = addRun(NO_RESULTS, run(result("a")), { append: false });
    const two = addRun(one, run(result("b"), result("c", { error: err })), { append: true });
    expect(two.active).toBe(2);
    const closed = closeResult(two, 0);
    expect(closed.items.map((i) => i.result.sql)).toEqual(["b", "c"]);
    expect(closed.active).toBe(1);
  });
});
