import { describe, expect, it } from "vitest";
import { type StatementResult } from "@/types";
import { resultFiles } from "./exportAllResults";
import { type ResultItem } from "./results";

const result = (extra: Partial<StatementResult>): StatementResult => ({
  sql: "SELECT 1",
  columns: [{ name: "a", typeName: "int" }] as StatementResult["columns"],
  rows: [[1], [null]],
  totalRows: 2,
  truncated: false,
  rowsAffected: null,
  durationMs: 1,
  error: null,
  ...extra,
});
const item = (key: string, r: StatementResult): ResultItem => ({ key, run: 1, result: r });

describe("export results from all tabs", () => {
  it("makes one file per result with rows", () => {
    const items = [
      item("1", result({})),
      item("2", result({ columns: [], rows: [], rowsAffected: 3 })),
      item("3", result({ rows: [[2]] })),
    ];
    expect(resultFiles(items, "csv", "postgres")).toEqual([
      { name: "result-1.csv", contents: "a\n1\n" },
      { name: "result-2.csv", contents: "a\n2" },
    ]);
    expect(resultFiles(items, "markdown", "postgres")[1]?.name).toBe("result-2.md");
  });
});
