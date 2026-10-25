import { describe, expect, it } from "vitest";
import { type StatementResult } from "@/types";
import {
  arcPath,
  band,
  chartData,
  extent,
  formatValue,
  linear,
  niceTicks,
  pieSlices,
} from "./chartScale";
import {
  boardVariables,
  moveWidget,
  newBoard,
  newWidget,
  resizeWidget,
  variableNames,
} from "./boardModel";
import { DEFAULT_OPTIONS } from "./boardModel";

const result = (columns: string[], rows: StatementResult["rows"]): StatementResult => ({
  sql: "",
  columns: columns.map((name) => ({ name, typeName: "" })),
  rows,
  totalRows: rows.length,
  truncated: false,
  rowsAffected: null,
  durationMs: 1,
  error: null,
});

describe("chart scales", () => {
  it("makes round ticks that include zero", () => {
    expect(niceTicks(0, 97).ticks).toEqual([0, 20, 40, 60, 80, 100]);
    const neg = niceTicks(-12, 30, 4);
    expect([neg.min, neg.max, neg.step]).toEqual([-20, 40, 20]);
    expect(niceTicks(0, 0).max).toBe(1);
    expect(niceTicks(0, 0.7).ticks.at(-1)).toBeCloseTo(0.8);
  });

  it("maps values linearly and splits bands", () => {
    const y = linear(0, 100, 200, 0);
    expect([y(0), y(50), y(100)]).toEqual([200, 100, 0]);
    const b = band(4, 400, 0.2);
    expect(b.step).toBe(100);
    expect(b.width).toBe(80);
    expect(b.x(1)).toBe(110);
  });

  it("turns results into series", () => {
    const r = result(
      ["month", "sales", "note", "cost"],
      [
        ["Jan", 10, "x", "4.5"],
        ["Feb", null, "y", 6],
      ],
    );
    const d = chartData(r, DEFAULT_OPTIONS);
    expect(d.labels).toEqual(["Jan", "Feb"]);
    expect(d.series.map((s) => s.name)).toEqual(["sales", "cost"]);
    expect(d.series[1]?.values).toEqual([4.5, 6]);
    expect(extent(d.series)).toEqual([4.5, 10]);
    const only = chartData(r, { ...DEFAULT_OPTIONS, labelColumn: "note", valueColumns: ["cost"] });
    expect(only.labels).toEqual(["x", "y"]);
    expect(only.series).toHaveLength(1);
  });

  it("builds pie slices and arcs", () => {
    const s = pieSlices(["a", "b", "c"], [1, 3, -2]);
    expect(s.map((x) => x.label)).toEqual(["a", "b"]);
    expect(s[1]?.end).toBeCloseTo(Math.PI * 2);
    expect(s[0]?.end).toBeCloseTo(Math.PI / 2);
    expect(arcPath(50, 50, 40, 20, 0, Math.PI / 2)).toMatch(/^M 50\.00 10\.00 A 40 40 0 0 1 90/);
  });

  it("formats values", () => {
    expect(formatValue(1234.567, "number")).toBe("1,234.57");
    expect(formatValue(2_500_000, "auto")).toBe("2.5M");
    expect(formatValue(0.256, "percent")).toBe("25.6%");
    expect(formatValue(2048, "bytes")).toBe("2 KB");
    expect(formatValue(null, "auto")).toBe("–");
  });
});

describe("board model", () => {
  it("reorders, resizes and names input variables", () => {
    let b = newBoard("Sales");
    const a = newWidget("bar");
    const input = newWidget("input", ["value1"]);
    b = { ...b, widgets: [a, input] };
    expect(input.options.variable).toBe("value2");
    b = moveWidget(b, input.id, -1);
    expect(b.widgets[0]?.id).toBe(input.id);
    expect(moveWidget(b, input.id, -1)).toBe(b);
    b = resizeWidget(b, a.id, 5, -5);
    expect([b.widgets[1]?.width, b.widgets[1]?.height]).toEqual([4, 1]);
    expect(variableNames("select {{ region }}, {{n}}, {{region}}, {{ bad name }}")).toEqual([
      "region",
      "n",
    ]);
    expect(boardVariables(b, {})).toEqual([{ name: "value2", value: "", kind: "text" }]);
    expect(boardVariables(b, { value2: "EU" })[0]?.value).toBe("EU");
  });
});
