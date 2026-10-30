import { describe, expect, it } from "vitest";
import { orderColumns, reorder } from "./columns";
import { inRange, navigate, rowRange, spanRange } from "./navigation";

const ctx = { focus: { row: 2, col: 1 }, ordered: [0, 1, 2], rowCount: 10, pageRows: 4 };
const key = (k: string, mods: { shiftKey?: boolean; mod?: boolean } = {}) => ({
  key: k,
  shiftKey: false,
  mod: false,
  ...mods,
});

describe("grid navigation", () => {
  it("moves like a spreadsheet", () => {
    expect(navigate(key("ArrowDown"), ctx)).toEqual({ row: 3, col: 1 });
    expect(navigate(key("ArrowDown", { mod: true }), ctx)).toEqual({ row: 9, col: 1 });
    expect(navigate(key("PageUp"), ctx)).toEqual({ row: 0, col: 1 });
    expect(navigate(key("Tab"), ctx)).toEqual({ row: 2, col: 2 });
    expect(navigate(key("Tab", { shiftKey: true }), ctx)).toEqual({ row: 2, col: 0 });
    expect(navigate(key("End"), ctx)).toEqual({ row: 2, col: 2 });
    expect(navigate(key("a"), ctx)).toBeNull();
  });

  it("follows the display order of the columns", () => {
    expect(navigate(key("ArrowRight"), { ...ctx, ordered: [2, 1, 0] })).toEqual({ row: 2, col: 0 });
  });

  it("selects row ranges in either direction", () => {
    expect([...rowRange(4, 2)]).toEqual([2, 3, 4]);
  });
});

describe("column order", () => {
  const cols = ["a", "b", "c"].map((name) => ({ name, typeName: "text" }));

  it("puts saved columns first and new ones after", () => {
    expect(orderColumns(cols, ["c", "gone", "a"])).toEqual([2, 0, 1]);
  });

  it("drops a column before another", () => {
    expect(reorder(["a", "b", "c"], "c", "a")).toEqual(["c", "a", "b"]);
  });
});

describe("blocks of cells", () => {
  it("spans the rows and the displayed columns between two corners", () => {
    const ordered = [2, 0, 1];
    expect(spanRange({ row: 3, col: 1 }, { row: 1, col: 0 }, ordered)).toEqual({
      top: 1,
      bottom: 3,
      cols: [0, 1],
    });
    expect(spanRange({ row: 1, col: 0 }, { row: 1, col: 0 }, ordered)).toBeNull();
    const r = spanRange({ row: 0, col: 2 }, { row: 1, col: 0 }, ordered);
    expect(inRange(r, 1, 2)).toBe(true);
    expect(inRange(r, 1, 1)).toBe(false);
  });
});
