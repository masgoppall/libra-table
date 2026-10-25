import { describe, expect, it } from "vitest";
import { type FilePreview, type ImportSummary } from "@/types";
import {
  autoMatch,
  buildRequest,
  createTableChange,
  DEFAULT_CSV,
  DEFAULT_OPTIONS,
  formatOf,
  mappingProblem,
  newTableColumns,
  newTableProblem,
  newTableTargets,
  progressPercent,
  summaryOf,
  tableNameFrom,
} from "./importModel";

const preview: FilePreview = {
  columns: [
    { name: "ID", kind: "integer", suggestedType: "integer", nullable: false },
    { name: "Full Name", kind: "text", suggestedType: "text", nullable: true },
    { name: "extra", kind: "text", suggestedType: "text", nullable: true },
  ],
  rows: [["1", "Ann", null]],
  sampled: 1,
  totalBytes: 20,
  errors: [],
};

const summary = (s: Partial<ImportSummary>): ImportSummary => ({
  inserted: 0,
  skipped: 0,
  rowsRead: 0,
  errors: [],
  errorCount: 0,
  cancelled: false,
  stopped: false,
  rolledBack: false,
  fatal: null,
  ...s,
});

describe("import model", () => {
  it("guesses the format and a table name from the file", () => {
    expect(formatOf("/x/data.JSON")).toBe("json");
    expect(formatOf("/x/data.tsv")).toBe("csv");
    expect(tableNameFrom("/x/Sales Report 2024.csv")).toBe("sales_report_2024");
    expect(tableNameFrom("C:\\d\\2024.csv")).toBe("t_2024");
  });

  it("matches columns by name, ignoring case and punctuation, each target once", () => {
    // The exact name wins over a looser match.
    expect(autoMatch(["ID", "Full Name", "id", "x"], ["id", "full_name", "note"])).toEqual([
      null,
      "full_name",
      "id",
      null,
    ]);
    expect(autoMatch(["ID", "e-mail"], ["id", "email"])).toEqual(["id", "email"]);
    expect(autoMatch(["id"], ["id"])).toEqual(["id"]);
  });

  it("validates mappings and new tables", () => {
    expect(mappingProblem([null, null])).toMatch(/at least one/);
    expect(mappingProblem(["a", "a"])).toMatch(/twice/);
    expect(mappingProblem(["a", null])).toBeNull();
    const cols = newTableColumns(preview);
    expect(newTableProblem("", cols)).toMatch(/Name/);
    expect(newTableProblem("t", cols)).toBeNull();
    expect(
      newTableProblem("t", [
        { ...cols[0]!, name: "x" },
        { ...cols[1]!, name: "x" },
      ]),
    ).toMatch(/twice/);
  });

  it("builds the request and the CREATE TABLE of a new table", () => {
    const cols = newTableColumns(preview).map((c, i) => (i === 2 ? { ...c, include: false } : c));
    const targets = newTableTargets(cols);
    expect(targets).toEqual(["ID", "Full Name", null]);
    const source = { path: "/a.csv", format: "csv" as const, csv: DEFAULT_CSV };
    const where = { schema: "public", table: "people" };
    const req = buildRequest(source, preview, where, targets, { ...DEFAULT_OPTIONS, batchSize: 0 });
    expect(req.mapping).toEqual([
      { source: 0, target: "ID" },
      { source: 1, target: "Full Name" },
    ]);
    expect(req.sourceColumns).toEqual(["ID", "Full Name", "extra"]);
    expect(req.batchSize).toBe(1);
    const change = createTableChange("public", " people ", cols);
    expect(change.name).toBe("people");
    expect(change.columns.map((c) => [c.name, c.dataType, c.nullable])).toEqual([
      ["ID", "integer", true],
      ["Full Name", "text", true],
    ]);
  });

  it("describes progress and outcomes", () => {
    expect(progressPercent({ bytesRead: 50, totalBytes: 200 })).toBe(25);
    expect(progressPercent({ bytesRead: 5, totalBytes: 0 })).toBe(0);
    expect(summaryOf(summary({ inserted: 3 }))).toEqual({ tone: "ok", text: "3 rows imported." });
    const stopped = summaryOf(
      summary({
        stopped: true,
        rolledBack: true,
        errors: [{ row: 7, line: 8, message: "bad" }],
      }),
    );
    expect(stopped.tone).toBe("error");
    expect(stopped.text).toContain("row 7: bad");
    expect(summaryOf(summary({ inserted: 2, skipped: 1 })).tone).toBe("warn");
  });
});
