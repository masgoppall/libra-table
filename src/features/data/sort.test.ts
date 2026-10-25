import { describe, expect, it } from "vitest";
import { nextSort } from "./sort";

describe("header sort", () => {
  it("cycles ascending, descending, off", () => {
    const asc = nextSort([], "a", false);
    expect(asc).toEqual([{ column: "a", desc: false }]);
    const desc = nextSort(asc, "a", false);
    expect(desc).toEqual([{ column: "a", desc: true }]);
    expect(nextSort(desc, "a", false)).toEqual([]);
  });

  it("Shift adds a column and keeps the others", () => {
    const two = nextSort([{ column: "a", desc: false }], "b", true);
    expect(two.map((s) => s.column)).toEqual(["a", "b"]);
  });
});
